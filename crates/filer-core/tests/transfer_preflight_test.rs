use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use filer_core::model::session::SessionId;
use filer_core::modules::operations::OperationsModule;
use filer_core::{
    Capabilities, Command, CoreError, ErrorCode, Event, FilerCore, FsProvider, LocalFs, Location,
    LocationRef, NodeEntry, OperationId, ProviderCx, RequestId,
};
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(10);
const CONTENT: &[u8] = b"source bytes must survive rejection";

async fn isolated_case(name: &str) -> Option<tempfile::TempDir> {
    if std::env::var("FILER_TRANSFER_CASE").as_deref() == Ok(name) {
        return None;
    }
    let root = tempfile::tempdir().unwrap();
    let output = timeout(
        TIMEOUT,
        tokio::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("FILER_TRANSFER_CASE", name)
            .env("FILER_TRANSFER_ROOT", root.path())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("isolated transfer exceeded its deadline")
    .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Some(root)
}

fn local_ref(path: &Path) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

async fn transfer(sources: &[&Path], destination: &Path, moving: bool) -> Event {
    transfer_with_provider(
        sources,
        destination,
        moving,
        Arc::new(LocalFs::new()),
        ErrorCode::InputInvalid,
    )
    .await
}

async fn transfer_with_provider(
    sources: &[&Path],
    destination: &Path,
    moving: bool,
    provider: Arc<dyn FsProvider>,
    error_code: ErrorCode,
) -> Event {
    let core = FilerCore::new();
    core.load(OperationsModule::new(provider));
    let events = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    let session = match timeout(TIMEOUT, events.recv_async())
        .await
        .unwrap()
        .unwrap()
    {
        Event::SessionCreated(session) => session,
        other => panic!("expected SessionCreated, got {other:?}"),
    };
    let request = RequestId::new();
    let operation = OperationId::new();
    let sources = sources.iter().map(|path| local_ref(path)).collect();
    let destination = local_ref(destination);
    let command = if moving {
        Command::Move {
            sources,
            destination,
            session,
            request,
            operation,
        }
    } else {
        Command::Copy {
            sources,
            destination,
            session,
            request,
            operation,
        }
    };
    core.send(command).unwrap();
    timeout(TIMEOUT, async {
        loop {
            match events.recv_async().await.unwrap() {
                event @ Event::Error { .. } => {
                    assert_rejection(&event, session, request, operation);
                    assert!(matches!(&event, Event::Error { code, .. } if *code == error_code));
                    return event;
                }
                event @ Event::OperationComplete { .. } => return event,
                _ => {}
            }
        }
    })
    .await
    .unwrap()
}

fn assert_rejection(event: &Event, session: SessionId, request: RequestId, operation: OperationId) {
    assert!(matches!(event, Event::Error {
        session: actual_session,
        request: Some(actual_request),
        operation: Some(actual_operation),
        ..
    } if *actual_session == session && *actual_request == request && *actual_operation == operation));
}

#[tokio::test]
async fn copy_rejects_own_parent_without_truncation() {
    if isolated_case("copy_rejects_own_parent_without_truncation")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("file.txt");
    std::fs::write(&source, CONTENT).unwrap();
    assert!(matches!(
        transfer(&[&source], &root, false).await,
        Event::Error { .. }
    ));
    assert_eq!(std::fs::read(source).unwrap(), CONTENT);
}

#[tokio::test]
async fn transfers_reject_descendants_before_creating_destinations() {
    if isolated_case("transfers_reject_descendants_before_creating_destinations")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    for moving in [false, true] {
        let source = root.join(if moving { "move" } else { "copy" });
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("file.txt"), CONTENT).unwrap();
        let destination = source.join("missing/nested");
        assert!(matches!(
            transfer(&[&source], &destination, moving).await,
            Event::Error { .. }
        ));
        assert!(!source.join("missing").exists());
        assert_eq!(std::fs::read(source.join("file.txt")).unwrap(), CONTENT);
    }
}

#[tokio::test]
async fn transfers_reject_descendants_after_missing_parent_components() {
    if isolated_case("transfers_reject_descendants_after_missing_parent_components")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("file.txt"), CONTENT).unwrap();
    let destination = root.join("missing/../source/nested");
    for moving in [false, true] {
        assert!(matches!(
            transfer(&[&source], &destination, moving).await,
            Event::Error { .. }
        ));
        assert!(!root.join("missing").exists());
        assert!(!source.join("nested").exists());
        assert_eq!(std::fs::read(source.join("file.txt")).unwrap(), CONTENT);
    }
}

#[tokio::test]
async fn copy_rejects_hard_link_target_without_changing_either_name() {
    if isolated_case("copy_rejects_hard_link_target_without_changing_either_name")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("file.txt");
    let destination = root.join("destination");
    std::fs::create_dir(&destination).unwrap();
    let alias = destination.join("file.txt");
    std::fs::write(&source, CONTENT).unwrap();
    std::fs::hard_link(&source, &alias).unwrap();
    for moving in [false, true] {
        assert!(matches!(
            transfer(&[&source], &destination, moving).await,
            Event::Error { .. }
        ));
        assert_eq!(std::fs::read(&source).unwrap(), CONTENT);
        assert_eq!(std::fs::read(&alias).unwrap(), CONTENT);
    }
}

#[tokio::test]
async fn transfers_reject_identical_directories_and_existing_descendants() {
    if isolated_case("transfers_reject_identical_directories_and_existing_descendants")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("source");
    let nested = source.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(source.join("file.txt"), CONTENT).unwrap();
    for moving in [false, true] {
        for destination in [&root, &source, &nested] {
            assert!(matches!(
                transfer(&[&source], destination, moving).await,
                Event::Error { .. }
            ));
            assert_eq!(std::fs::read(source.join("file.txt")).unwrap(), CONTENT);
            assert_eq!(std::fs::read_dir(&source).unwrap().count(), 2);
            assert_eq!(std::fs::read_dir(&nested).unwrap().count(), 0);
        }
    }
}

#[cfg(any(unix, windows))]
fn symlink(source: &Path, alias: &Path, directory: bool) -> bool {
    #[cfg(unix)]
    let result = {
        let _ = directory;
        std::os::unix::fs::symlink(source, alias)
    };
    #[cfg(windows)]
    let result = if directory {
        std::os::windows::fs::symlink_dir(source, alias)
    } else {
        std::os::windows::fs::symlink_file(source, alias)
    };
    #[cfg(windows)]
    if result
        .as_ref()
        .is_err_and(|error| error.raw_os_error() == Some(1314))
    {
        eprintln!("Skipping symlink case: Windows does not grant symlink creation privileges");
        return false;
    }
    result.unwrap();
    true
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn copy_rejects_symlink_target_without_changing_either_name() {
    if isolated_case("copy_rejects_symlink_target_without_changing_either_name")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("file.txt");
    let destination = root.join("destination");
    std::fs::create_dir(&destination).unwrap();
    let alias = destination.join("file.txt");
    std::fs::write(&source, CONTENT).unwrap();
    if !symlink(&source, &alias, false) {
        return;
    }
    assert!(matches!(
        transfer(&[&source], &destination, false).await,
        Event::Error { .. }
    ));
    assert_eq!(std::fs::read(&source).unwrap(), CONTENT);
    assert_eq!(std::fs::read(&alias).unwrap(), CONTENT);
    assert!(std::fs::symlink_metadata(alias).unwrap().is_symlink());
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn transfers_reject_symlinked_directory_ancestors_and_targets() {
    if isolated_case("transfers_reject_symlinked_directory_ancestors_and_targets")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("file.txt"), CONTENT).unwrap();
    let ancestor = root.join("alias");
    if !symlink(&source, &ancestor, true) {
        return;
    }
    let destination = root.join("destination");
    std::fs::create_dir(&destination).unwrap();
    if !symlink(&source, &destination.join("source"), true) {
        return;
    }
    for moving in [false, true] {
        for parent in [&ancestor, &ancestor.join("missing/nested"), &destination] {
            assert!(matches!(
                transfer(&[&source], parent, moving).await,
                Event::Error { .. }
            ));
            assert!(!source.join("missing").exists());
            assert_eq!(std::fs::read_dir(&source).unwrap().count(), 1);
            assert_eq!(std::fs::read(source.join("file.txt")).unwrap(), CONTENT);
        }
    }
}

#[tokio::test]
async fn invalid_later_source_rejects_entire_batch_before_overwriting() {
    if isolated_case("invalid_later_source_rejects_entire_batch_before_overwriting")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let file = root.join("file.txt");
    let directory = root.join("source");
    let destination = directory.join("nested");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(&file, CONTENT).unwrap();
    std::fs::write(destination.join("file.txt"), b"existing destination bytes").unwrap();
    for moving in [false, true] {
        assert!(matches!(
            transfer(&[&file, &directory], &destination, moving).await,
            Event::Error { .. }
        ));
        assert_eq!(std::fs::read(&file).unwrap(), CONTENT);
        assert_eq!(
            std::fs::read(destination.join("file.txt")).unwrap(),
            b"existing destination bytes"
        );
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 1);
    }
}

#[tokio::test]
async fn valid_file_and_directory_transfers_preserve_contents() {
    let root = tempfile::tempdir().unwrap();
    for moving in [false, true] {
        let source = root.path().join(if moving { "move" } else { "copy" });
        let destination = source.with_extension("destination");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::create_dir(&destination).unwrap();
        let file = root
            .path()
            .join(if moving { "move.txt" } else { "copy.txt" });
        std::fs::write(&file, CONTENT).unwrap();
        std::fs::write(source.join("nested/file.txt"), CONTENT).unwrap();
        assert!(matches!(
            transfer(&[&file, &source], &destination, moving).await,
            Event::OperationComplete { success: true, .. }
        ));
        assert_eq!(
            std::fs::read(destination.join(file.file_name().unwrap())).unwrap(),
            CONTENT
        );
        assert_eq!(
            std::fs::read(
                destination
                    .join(source.file_name().unwrap())
                    .join("nested/file.txt")
            )
            .unwrap(),
            CONTENT
        );
        assert_eq!(source.exists(), !moving);
        assert_eq!(file.exists(), !moving);
    }
}

struct UncheckedProvider;

#[async_trait::async_trait]
impl FsProvider for UncheckedProvider {
    fn scheme(&self) -> &'static str {
        "unchecked"
    }

    fn capabilities(&self) -> Capabilities {
        LocalFs::new().capabilities()
    }

    async fn list(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        LocalFs::new().list(path, cx).await
    }

    async fn read(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        LocalFs::new().read(path, cx).await
    }

    async fn read_range(
        &self,
        path: &Path,
        start: u64,
        len: u64,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        LocalFs::new().read_range(path, start, len, cx).await
    }

    async fn exists(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        LocalFs::new().exists(path, cx).await
    }

    async fn metadata(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<NodeEntry, CoreError> {
        LocalFs::new().metadata(path, cx).await
    }

    async fn copy(&self, src: &Path, dst: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        LocalFs::new().copy(src, dst, cx).await
    }

    async fn rename(&self, src: &Path, dst: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        LocalFs::new().rename(src, dst, cx).await
    }
}

#[tokio::test]
async fn provider_without_identity_checks_rejects_transfers_explicitly() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("file.txt");
    let destination = root.path().join("destination");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(&source, CONTENT).unwrap();
    for moving in [false, true] {
        let event = transfer_with_provider(
            &[&source],
            &destination,
            moving,
            Arc::new(UncheckedProvider),
            ErrorCode::UnsupportedOperation,
        )
        .await;
        assert!(matches!(event, Event::Error { .. }));
        assert_eq!(std::fs::read(&source).unwrap(), CONTENT);
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
    }
}
