use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, process::Command as ProcessCommand};

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::fs::symlink;

use async_trait::async_trait;
use tempfile::TempDir;
use tokio::sync::Notify;

use crate::model::fs_change::FsChangeKind;
use crate::model::session::SessionId;
use crate::modules::git_decorations::GitCliBackend;
use crate::modules::git_decorations::{
    FileDecoration, FileDecorationInvalidation, FileDecorationState, GitDecorationRequest,
    GitDecorationTarget, GitDecorationsModule, GitRepository, GitStatusBackend, GitStatusResult,
    MAX_VISIBLE_DECORATIONS,
};
use crate::vfs::watch::{FsChange, WatchHandle, WatchProvider};
use crate::{
    Command, DirectoryLoadOptions, Event, FilerCore, LocalFs, Location, LocationRef,
    PipelineConfig, RequestId,
};

struct TestWatchHandle;

impl WatchHandle for TestWatchHandle {}

#[derive(Default)]
struct TestWatchProvider {
    sender: Mutex<Option<flume::Sender<FsChange>>>,
    watched: Mutex<Vec<PathBuf>>,
}

impl TestWatchProvider {
    async fn emit(&self, path: PathBuf, kind: FsChangeKind) {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(sender) = sender {
            let _ = sender.send_async(FsChange { path, kind }).await;
        }
    }
}

#[async_trait]
impl WatchProvider for TestWatchProvider {
    async fn watch(
        &self,
        path: &Path,
        sender: flume::Sender<FsChange>,
    ) -> Result<Box<dyn WatchHandle>, crate::CoreError> {
        *self
            .sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(sender);
        self.watched
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(path.to_path_buf());
        Ok(Box::new(TestWatchHandle))
    }

    async fn unwatch(&self, path: &Path) -> Result<(), crate::CoreError> {
        self.watched
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|watched| watched != path);
        Ok(())
    }
}

struct StaticBackend {
    states: Vec<FileDecorationState>,
    gate: Option<Arc<Notify>>,
    calls: Arc<Mutex<Vec<Vec<LocationRef>>>>,
    common_dir: PathBuf,
}

#[async_trait]
impl GitStatusBackend for StaticBackend {
    async fn status(
        &self,
        parent: &Path,
        visible: &[GitDecorationTarget],
        cancel: &crate::CancelSignal,
    ) -> Result<GitStatusResult, crate::CoreError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(
                visible
                    .iter()
                    .map(|target| target.location.clone())
                    .collect(),
            );
        if let Some(gate) = &self.gate {
            tokio::select! {
                _ = gate.notified() => {}
                _ = cancel.cancelled() => return Err(crate::CoreError::cancelled()),
            }
        }
        let decorations = visible
            .iter()
            .enumerate()
            .map(|(index, target)| FileDecoration {
                location: target.location.clone(),
                state: self
                    .states
                    .get(index)
                    .copied()
                    .unwrap_or(FileDecorationState::Clean),
            })
            .collect();
        Ok(GitStatusResult {
            repository: Some(GitRepository {
                worktree: parent.to_path_buf(),
                git_dir: parent.join(".git"),
                common_dir: self.common_dir.clone(),
            }),
            decorations,
        })
    }
}

fn request(parent: &Location, visible: &[LocationRef]) -> GitDecorationRequest {
    GitDecorationRequest {
        parent: LocationRef::from_location(parent),
        visible: visible.to_vec(),
        request: RequestId::new(),
    }
}

async fn wait_for_event(events: &flume::Receiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(2), events.recv_async())
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for decoration event"))
        .unwrap_or_else(|_| panic!("event channel closed"))
}

async fn create_session(core: &FilerCore, events: &flume::Receiver<Event>) -> SessionId {
    core.send(Command::Handshake).unwrap();
    loop {
        if let Event::SessionCreated(session) = wait_for_event(events).await {
            return session;
        }
    }
}

#[tokio::test]
async fn semantic_states_are_emitted_by_location_identity() {
    let temp = TempDir::new().unwrap();
    let parent = Location::local(temp.path());
    let paths = [
        "modified",
        "added",
        "deleted",
        "untracked",
        "ignored",
        "conflicted",
        "clean",
    ];
    let visible: Vec<_> = paths
        .iter()
        .map(|name| LocationRef::from_location(&Location::local(temp.path().join(name))))
        .collect();
    let states = vec![
        FileDecorationState::Modified,
        FileDecorationState::Added,
        FileDecorationState::Deleted,
        FileDecorationState::Untracked,
        FileDecorationState::Ignored,
        FileDecorationState::Conflicted,
        FileDecorationState::Clean,
    ];
    let calls = Arc::new(Mutex::new(Vec::new()));
    let backend = Arc::new(StaticBackend {
        states: states.clone(),
        gate: None,
        calls: calls.clone(),
        common_dir: temp.path().join(".git"),
    });
    let watcher = Arc::new(TestWatchProvider::default());
    let core = FilerCore::new();
    core.load(GitDecorationsModule::with_components(backend, watcher));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;
    let request = request(&parent, &visible);
    let request_id = request.request;

    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(request),
        session,
    })
    .unwrap();

    let event = wait_for_event(&events).await;
    assert!(matches!(
        event,
        Event::FileDecorationsUpdated {
            decorations,
            session: event_session,
            request,
        } if event_session == session
            && request == request_id
            && decorations
                .iter()
                .map(|decoration| decoration.state)
                .eq(states.iter().copied())
            && decorations
                .iter()
                .zip(visible.iter())
                .all(|(decoration, location)| &decoration.location == location)
    ));
    assert_eq!(calls.lock().unwrap().len(), 1);
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn malformed_and_oversized_requests_are_recoverable_errors() {
    let temp = TempDir::new().unwrap();
    let watcher = Arc::new(TestWatchProvider::default());
    let backend = Arc::new(StaticBackend {
        states: Vec::new(),
        gate: None,
        calls: Arc::new(Mutex::new(Vec::new())),
        common_dir: temp.path().join(".git"),
    });
    let core = FilerCore::new();
    core.load(GitDecorationsModule::with_components(backend, watcher));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;
    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new("wrong payload".to_string()),
        session,
    })
    .unwrap();
    let malformed = wait_for_event(&events).await;
    assert!(
        matches!(malformed, Event::Error { session: event_session, .. } if event_session == session)
    );

    let parent = Location::local(temp.path());
    let visible: Vec<_> = (0..=MAX_VISIBLE_DECORATIONS)
        .map(|index| {
            LocationRef::from_location(&Location::local(temp.path().join(index.to_string())))
        })
        .collect();
    let request = request(&parent, &visible);
    let request_id = request.request;
    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(request),
        session,
    })
    .unwrap();
    let oversized = wait_for_event(&events).await;
    assert!(matches!(
        oversized,
        Event::Error { request: Some(event_request), code: crate::ErrorCode::InputInvalid, .. } if event_request == request_id
    ));
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn repository_changes_invalidate_matching_visible_rows() {
    let temp = TempDir::new().unwrap();
    let parent = Location::local(temp.path());
    let changed = Location::local(temp.path().join("changed.txt"));
    let untouched = Location::local(temp.path().join("untouched.txt"));
    let visible = vec![
        LocationRef::from_location(&changed),
        LocationRef::from_location(&untouched),
    ];
    let watcher = Arc::new(TestWatchProvider::default());
    let backend = Arc::new(StaticBackend {
        states: vec![FileDecorationState::Modified, FileDecorationState::Clean],
        gate: None,
        calls: Arc::new(Mutex::new(Vec::new())),
        common_dir: temp.path().join(".git"),
    });
    let core = FilerCore::new();
    core.load(GitDecorationsModule::with_components(
        backend,
        watcher.clone(),
    ));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;
    let request = request(&parent, &visible);
    let request_id = request.request;
    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(request),
        session,
    })
    .unwrap();
    let _ = wait_for_event(&events).await;

    watcher
        .emit(
            changed.as_local_path().unwrap().to_path_buf(),
            FsChangeKind::Modified,
        )
        .await;
    let event = wait_for_event(&events).await;
    assert!(matches!(
        event,
        Event::FileDecorationsInvalidated {
            invalidation: FileDecorationInvalidation { locations },
            session: event_session,
            request: event_request,
        } if event_session == session
            && event_request == request_id
            && locations == vec![LocationRef::from_location(&changed)]
    ));

    core.send(Command::DestroySession(session)).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(watcher.watched.lock().unwrap().is_empty());
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_new_request_cancels_the_previous_request() {
    let temp = TempDir::new().unwrap();
    let parent = Location::local(temp.path());
    let first = LocationRef::from_location(&Location::local(temp.path().join("first")));
    let second = LocationRef::from_location(&Location::local(temp.path().join("second")));
    let gate = Arc::new(Notify::new());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let backend = Arc::new(StaticBackend {
        states: vec![FileDecorationState::Clean],
        gate: Some(gate.clone()),
        calls: calls.clone(),
        common_dir: temp.path().join(".git"),
    });
    let watcher = Arc::new(TestWatchProvider::default());
    let core = FilerCore::new();
    core.load(GitDecorationsModule::with_components(backend, watcher));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;
    let first_request = request(&parent, &[first]);
    let second_request = request(&parent, &[second]);
    let second_id = second_request.request;
    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(first_request),
        session,
    })
    .unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !calls
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_empty()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(second_request),
        session,
    })
    .unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if calls
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len()
                >= 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    gate.notify_waiters();
    let event = wait_for_event(&events).await;
    assert!(matches!(event, Event::FileDecorationsUpdated { request, .. } if request == second_id));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), events.recv_async())
            .await
            .is_err()
    );
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn git_cli_maps_all_supported_states() {
    let temp = TempDir::new().unwrap();
    run_git(temp.path(), ["init", "-q"]);
    run_git(temp.path(), ["branch", "-M", "main"]);
    run_git(temp.path(), ["config", "user.email", "filer@example.test"]);
    run_git(temp.path(), ["config", "user.name", "Filer Test"]);
    fs::write(temp.path().join(".gitignore"), "ignored.txt\n").unwrap();
    for name in ["clean.txt", "modified.txt", "deleted.txt", "conflicted.txt"] {
        fs::write(temp.path().join(name), "base\n").unwrap();
    }
    run_git(temp.path(), ["add", "."]);
    run_git(temp.path(), ["commit", "-qm", "base"]);

    run_git(temp.path(), ["checkout", "-qb", "feature"]);
    fs::write(temp.path().join("conflicted.txt"), "feature\n").unwrap();
    run_git(temp.path(), ["commit", "-qam", "feature"]);
    run_git(temp.path(), ["checkout", "-q", "main"]);
    fs::write(temp.path().join("conflicted.txt"), "main\n").unwrap();
    run_git(temp.path(), ["commit", "-qam", "main change"]);
    let merge = ProcessCommand::new("git")
        .args(["-C", temp.path().to_str().unwrap(), "merge", "feature"])
        .output()
        .unwrap();
    assert!(
        !merge.status.success(),
        "the merge should create a conflict"
    );

    fs::write(temp.path().join("modified.txt"), "changed\n").unwrap();
    fs::remove_file(temp.path().join("deleted.txt")).unwrap();
    fs::write(temp.path().join("added.txt"), "added\n").unwrap();
    run_git(temp.path(), ["add", "added.txt"]);
    fs::write(temp.path().join("untracked.txt"), "untracked\n").unwrap();
    fs::write(temp.path().join("ignored.txt"), "ignored\n").unwrap();

    let names = [
        "modified.txt",
        "added.txt",
        "deleted.txt",
        "untracked.txt",
        "ignored.txt",
        "conflicted.txt",
        "clean.txt",
    ];
    let locations: Vec<_> = names
        .iter()
        .map(|name| LocationRef::from_location(&Location::local(temp.path().join(name))))
        .collect();
    let targets: Vec<_> = locations
        .iter()
        .map(|location| GitDecorationTarget {
            location: location.clone(),
            path: location.descriptor().unwrap().root().to_path_buf(),
        })
        .collect();
    let result = GitCliBackend::new()
        .status(temp.path(), &targets, &crate::CancelSignal::new())
        .await
        .unwrap();
    let states: Vec<_> = result
        .decorations
        .iter()
        .map(|decoration| decoration.state)
        .collect();
    assert_eq!(
        states,
        vec![
            FileDecorationState::Modified,
            FileDecorationState::Added,
            FileDecorationState::Deleted,
            FileDecorationState::Untracked,
            FileDecorationState::Ignored,
            FileDecorationState::Conflicted,
            FileDecorationState::Clean,
        ]
    );

    let root_location = LocationRef::from_location(&Location::local(temp.path()));
    let root_result = GitCliBackend::new()
        .status(
            temp.path(),
            &[GitDecorationTarget {
                location: root_location,
                path: temp.path().to_path_buf(),
            }],
            &crate::CancelSignal::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        root_result.decorations[0].state,
        FileDecorationState::Conflicted
    );
}

#[tokio::test]
async fn git_cli_reports_non_repository_as_empty_and_missing_program_as_error() {
    let temp = TempDir::new().unwrap();
    let location = Location::local(temp.path().join("file.txt"));
    let target = GitDecorationTarget {
        location: LocationRef::from_location(&location),
        path: location.as_local_path().unwrap().to_path_buf(),
    };
    let cancel = crate::CancelSignal::new();

    let outside = GitCliBackend::new()
        .status(temp.path(), std::slice::from_ref(&target), &cancel)
        .await
        .unwrap();
    assert!(outside.repository.is_none());
    assert!(outside.decorations.is_empty());

    let missing = GitCliBackend::with_program(temp.path().join("missing-git"))
        .status(temp.path(), &[target], &cancel)
        .await
        .unwrap_err();
    assert_eq!(missing.code(), crate::ErrorCode::UnsupportedOperation);
}

#[cfg(unix)]
#[tokio::test]
async fn git_cli_accepts_a_repository_opened_through_a_symlink() {
    let temp = TempDir::new().unwrap();
    let real = temp.path().join("real");
    fs::create_dir(&real).unwrap();
    run_git(&real, ["init", "-q"]);
    let link = temp.path().join("link");
    symlink(&real, &link).unwrap();
    let target_path = link.join("visible.txt");
    fs::write(&target_path, b"untracked").unwrap();
    let target = GitDecorationTarget {
        location: LocationRef::from_location(&Location::local(&target_path)),
        path: target_path,
    };

    let result = GitCliBackend::new()
        .status(&link, &[target], &crate::CancelSignal::new())
        .await
        .unwrap();

    assert_eq!(result.decorations[0].state, FileDecorationState::Untracked);
}

#[tokio::test]
async fn git_cli_preserves_repository_path_whitespace() {
    let temp = TempDir::new().unwrap();
    let repository = temp.path().join("repository ");
    fs::create_dir(&repository).unwrap();
    run_git(&repository, ["init", "-q"]);
    let path = repository.join("visible.txt");
    fs::write(&path, b"untracked").unwrap();
    let target = GitDecorationTarget {
        location: LocationRef::from_location(&Location::local(&path)),
        path,
    };

    let result = GitCliBackend::new()
        .status(&repository, &[target], &crate::CancelSignal::new())
        .await
        .unwrap();

    assert_eq!(result.decorations[0].state, FileDecorationState::Untracked);
}

#[cfg(unix)]
#[tokio::test]
async fn git_cli_preserves_non_utf8_filenames() {
    let temp = TempDir::new().unwrap();
    run_git(temp.path(), ["init", "-q"]);
    let name = OsString::from_vec(b"non_utf8_\xff.txt".to_vec());
    let path = temp.path().join(&name);
    fs::write(&path, b"untracked").unwrap();
    let target = GitDecorationTarget {
        location: LocationRef::from_location(&Location::local(&path)),
        path,
    };

    let result = GitCliBackend::new()
        .status(temp.path(), &[target], &crate::CancelSignal::new())
        .await
        .unwrap();

    assert_eq!(result.decorations[0].state, FileDecorationState::Untracked);
}

#[tokio::test]
async fn linked_worktree_status_watches_the_shared_git_directory() {
    let temp = TempDir::new().unwrap();
    let parent = Location::local(temp.path());
    let shared_git = temp.path().join("shared-git");
    let location = Location::local(temp.path().join("visible"));
    let target = LocationRef::from_location(&location);
    let watcher = Arc::new(TestWatchProvider::default());
    let backend = Arc::new(StaticBackend {
        states: vec![FileDecorationState::Clean],
        gate: None,
        calls: Arc::new(Mutex::new(Vec::new())),
        common_dir: shared_git.clone(),
    });
    let core = FilerCore::new();
    core.load(GitDecorationsModule::with_components(
        backend,
        watcher.clone(),
    ));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;
    let request = request(&parent, &[target]);
    let request_id = request.request;

    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(request),
        session,
    })
    .unwrap();
    let _ = wait_for_event(&events).await;

    let watched = watcher
        .watched
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    assert!(watched.contains(&shared_git));

    watcher
        .emit(
            shared_git.join("refs").join("heads").join("linked"),
            FsChangeKind::Modified,
        )
        .await;
    assert!(matches!(
        wait_for_event(&events).await,
        Event::FileDecorationsInvalidated {
            invalidation: FileDecorationInvalidation { locations },
            session: event_session,
            request: event_request,
        } if event_session == session
            && event_request == request_id
            && locations.len() == 1
    ));
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn git_cli_reports_the_shared_directory_for_a_linked_worktree() {
    let temp = TempDir::new().unwrap();
    let primary = temp.path().join("primary");
    let linked = temp.path().join("linked");
    fs::create_dir(&primary).unwrap();
    run_git(&primary, ["init", "-q"]);
    run_git(&primary, ["config", "user.email", "filer@example.test"]);
    run_git(&primary, ["config", "user.name", "Filer Test"]);
    fs::write(primary.join("base.txt"), b"base").unwrap();
    run_git(&primary, ["add", "."]);
    run_git(&primary, ["commit", "-qm", "base"]);
    let output = ProcessCommand::new("git")
        .args([
            "-C",
            primary.to_str().unwrap(),
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
        ])
        .arg(&linked)
        .output()
        .unwrap();
    assert!(output.status.success());

    let result = GitCliBackend::new()
        .status(&linked, &[], &crate::CancelSignal::new())
        .await
        .unwrap();

    let repository = result.repository.unwrap();
    assert_eq!(repository.worktree, linked.canonicalize().unwrap());
    assert_eq!(
        repository.common_dir,
        primary.join(".git").canonicalize().unwrap()
    );
}

#[tokio::test]
async fn first_directory_page_arrives_before_gated_decoration_work() {
    const ENTRY_COUNT: usize = 10_000;
    const PAGE_SIZE: usize = 256;
    let temp = TempDir::new().unwrap();
    run_git(temp.path(), ["init", "-q"]);
    for index in 0..ENTRY_COUNT {
        fs::File::create(temp.path().join(format!("entry_{index:05}.txt"))).unwrap();
    }
    let parent = Location::local(temp.path());
    let visible: Vec<_> = (0..PAGE_SIZE)
        .map(|index| {
            LocationRef::from_location(&Location::local(
                temp.path().join(format!("entry_{index:05}.txt")),
            ))
        })
        .collect();
    let gate = Arc::new(Notify::new());
    let backend = Arc::new(StaticBackend {
        states: vec![FileDecorationState::Clean; PAGE_SIZE],
        gate: Some(gate.clone()),
        calls: Arc::new(Mutex::new(Vec::new())),
        common_dir: temp.path().join(".git"),
    });
    let core = FilerCore::new();
    core.load(crate::modules::scan::ScanModule::new(Arc::new(
        LocalFs::new(),
    )));
    core.load(GitDecorationsModule::with_backend(backend));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;
    let scan_request = RequestId::new();
    let decoration_request = request(&parent, &visible);
    let decoration_request_id = decoration_request.request;

    core.send(Command::Scan {
        location: LocationRef::from_location(&parent),
        session,
        pipeline: PipelineConfig::default(),
        load: DirectoryLoadOptions::page(PAGE_SIZE),
        request: scan_request,
    })
    .unwrap();
    core.send(Command::Extension {
        key: "git.status".to_string(),
        payload: Arc::new(decoration_request),
        session,
    })
    .unwrap();

    loop {
        let event = wait_for_event(&events).await;
        match event {
            Event::DirectoryPageLoaded { request, .. } if request == scan_request => break,
            Event::FileDecorationsUpdated { request, .. } if request == decoration_request_id => {
                panic!("decoration work blocked the first directory page")
            }
            _ => {}
        }
    }
    gate.notify_waiters();
    loop {
        if let Event::FileDecorationsUpdated { request, .. } = wait_for_event(&events).await
            && request == decoration_request_id
        {
            break;
        }
    }
    core.shutdown().await.unwrap();
}

fn run_git<const N: usize>(directory: &Path, args: [&str; N]) {
    let output = ProcessCommand::new("git")
        .args(["-C", directory.to_str().unwrap()])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
