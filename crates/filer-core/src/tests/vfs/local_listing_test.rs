//! Tests that local listings enumerate each page in one blocking task while
//! keeping the rows the per-entry `tokio::fs` path produced.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Instant, SystemTime};

use tempfile::TempDir;

use crate::errors::ErrorCode;
use crate::model::cancel::CancelSignal;
use crate::model::directory::DirectoryPageRequest;
use crate::model::node::{NodeEntry, NodeKind};
use crate::vfs::context::ProviderCx;
use crate::vfs::local::LocalFs;
use crate::vfs::local_listing::{ListingStop, read_batch};
use crate::vfs::provider::{FsProvider, ListingDetail, ListingOptions};

/// Every field a listing row exposes, in a comparable form.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RowSignature {
    name: String,
    kind: String,
    size: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    accessed: Option<SystemTime>,
    hidden: bool,
    readonly: bool,
    permissions: Option<u32>,
    navigate: bool,
}

fn signature(entry: &NodeEntry) -> RowSignature {
    // Resolving a link target updates the link's own access time on some
    // filesystems, so every listing would see a different value.
    let accessed = match entry.kind {
        NodeKind::Symlink { .. } => None,
        _ => entry.accessed,
    };
    RowSignature {
        name: entry.name.clone(),
        kind: format!("{:?}", entry.kind),
        size: entry.size,
        modified: entry.modified,
        created: entry.created,
        accessed,
        hidden: entry.meta.hidden,
        readonly: entry.meta.readonly,
        permissions: entry.meta.permissions,
        navigate: entry.capabilities.navigate,
    }
}

fn signatures(entries: &[NodeEntry]) -> Vec<RowSignature> {
    let mut rows: Vec<RowSignature> = entries.iter().map(signature).collect();
    rows.sort();
    rows
}

fn options(detail: ListingDetail) -> ListingOptions {
    match detail {
        ListingDetail::Fast => ListingOptions::fast(),
        ListingDetail::Metadata => ListingOptions::metadata(),
    }
}

/// A directory holding every row kind the classification code distinguishes.
fn mixed_directory() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("sized.txt"), b"0123456789").unwrap();
    std::fs::write(root.join("empty"), b"").unwrap();
    std::fs::write(root.join("báo cáo.tài liệu"), b"non-ascii").unwrap();
    std::fs::create_dir(root.join("folder")).unwrap();
    write_hidden_file(root);
    write_links(root);
    dir
}

#[cfg(unix)]
fn write_hidden_file(root: &Path) {
    std::fs::write(root.join(".hidden"), b"hidden").unwrap();
}

#[cfg(windows)]
fn write_hidden_file(root: &Path) {
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .attributes(FILE_ATTRIBUTE_HIDDEN)
        .open(root.join("hidden.txt"))
        .unwrap();
    file.write_all(b"hidden").unwrap();
}

#[cfg(unix)]
fn write_links(root: &Path) {
    std::os::unix::fs::symlink(root.join("sized.txt"), root.join("file-link")).unwrap();
    std::os::unix::fs::symlink(root.join("folder"), root.join("folder-link")).unwrap();
    std::os::unix::fs::symlink(root.join("missing"), root.join("dangling-link")).unwrap();
}

/// Junctions need no symlink privilege, so this runs on stock Windows accounts.
#[cfg(windows)]
fn write_links(root: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(root.join("folder-junction"))
        .arg(root.join("folder"))
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J should create a junction");
}

/// The rows the per-entry `tokio::fs` listing produced before batching.
async fn reference_rows(path: &Path, detail: ListingDetail) -> Vec<RowSignature> {
    let mut dir = tokio::fs::read_dir(path).await.unwrap();
    let mut entries = Vec::new();
    while let Some(entry) = dir.next_entry().await.unwrap() {
        let row = match detail {
            ListingDetail::Fast => {
                NodeEntry::from_dir_entry(entry.path(), entry.file_type().await.unwrap())
            }
            ListingDetail::Metadata => {
                NodeEntry::from_metadata(entry.metadata().await.unwrap(), entry.path()).unwrap()
            }
        };
        entries.push(row);
    }
    signatures(&entries)
}

async fn stream_rows(
    fs: &LocalFs,
    path: &Path,
    detail: ListingDetail,
    batch: usize,
) -> Vec<NodeEntry> {
    let mut stream = fs
        .open_listing(path, options(detail), &ProviderCx::none())
        .await
        .unwrap()
        .expect("the local provider should expose a listing stream");
    let mut rows = Vec::new();
    loop {
        let page = stream.next_batch(batch, &ProviderCx::none()).await.unwrap();
        rows.extend(page.entries);
        if page.end_of_directory {
            return rows;
        }
    }
}

async fn paged_rows(
    fs: &LocalFs,
    path: &Path,
    detail: ListingDetail,
    limit: usize,
) -> Vec<NodeEntry> {
    let mut rows = Vec::new();
    let mut cursor = None;
    loop {
        let page = fs
            .list_page(
                path,
                DirectoryPageRequest {
                    listing: options(detail),
                    limit,
                    cursor,
                },
                &ProviderCx::none(),
            )
            .await
            .unwrap();
        rows.extend(page.entries);
        if page.state.complete {
            assert!(page.state.next_cursor.is_none());
            return rows;
        }
        cursor = page.state.next_cursor;
        assert!(cursor.is_some(), "an incomplete page should carry a cursor");
    }
}

async fn assert_every_listing_matches_reference(detail: ListingDetail) {
    let fs = LocalFs::new();
    let dir = mixed_directory();
    let expected = reference_rows(dir.path(), detail).await;

    let listed = fs
        .list_with_options(dir.path(), options(detail), &ProviderCx::none())
        .await
        .unwrap();
    assert_eq!(signatures(&listed), expected, "list_with_options");
    assert_eq!(
        signatures(&stream_rows(&fs, dir.path(), detail, 2).await),
        expected,
        "listing stream"
    );
    assert_eq!(
        signatures(&paged_rows(&fs, dir.path(), detail, 2).await),
        expected,
        "offset pages"
    );
}

#[tokio::test]
async fn test_local_metadata_listings_match_per_entry_metadata_rows() {
    assert_every_listing_matches_reference(ListingDetail::Metadata).await;
}

#[tokio::test]
async fn test_local_fast_listings_match_per_entry_file_type_rows() {
    assert_every_listing_matches_reference(ListingDetail::Fast).await;
}

#[cfg(unix)]
#[tokio::test]
async fn test_local_metadata_listing_classifies_links_without_following_them() {
    let fs = LocalFs::new();
    let dir = mixed_directory();

    let rows = fs
        .list_with_options(dir.path(), ListingOptions::metadata(), &ProviderCx::none())
        .await
        .unwrap();
    let kind_of = |name: &str| {
        let row = rows.iter().find(|row| row.name == name).unwrap();
        format!("{:?}", row.kind)
    };

    assert!(kind_of("file-link").starts_with("Symlink"));
    assert!(kind_of("folder-link").starts_with("Symlink"));
    assert!(kind_of("dangling-link").starts_with("Symlink"));
    assert!(
        rows.iter()
            .find(|row| row.name == ".hidden")
            .unwrap()
            .meta
            .hidden
    );
}

#[cfg(windows)]
#[tokio::test]
async fn test_local_metadata_listing_classifies_junctions_and_hidden_attributes() {
    let fs = LocalFs::new();
    let dir = mixed_directory();

    let rows = fs
        .list_with_options(dir.path(), ListingOptions::metadata(), &ProviderCx::none())
        .await
        .unwrap();
    let junction = rows
        .iter()
        .find(|row| row.name == "folder-junction")
        .unwrap();

    assert!(format!("{:?}", junction.kind).starts_with("Symlink"));
    assert!(
        rows.iter()
            .find(|row| row.name == "hidden.txt")
            .unwrap()
            .meta
            .hidden
    );
}

#[tokio::test]
async fn test_local_list_page_cursor_yields_every_entry_once() {
    let fs = LocalFs::new();
    let dir = tempfile::tempdir().unwrap();
    for index in 0..7 {
        std::fs::write(dir.path().join(format!("entry-{index}.txt")), b"entry").unwrap();
    }

    for detail in [ListingDetail::Fast, ListingDetail::Metadata] {
        let mut names: Vec<String> = paged_rows(&fs, dir.path(), detail, 2)
            .await
            .into_iter()
            .map(|row| row.name)
            .collect();
        names.sort();
        let expected: Vec<String> = (0..7).map(|index| format!("entry-{index}.txt")).collect();
        assert_eq!(names, expected);
    }
}

/// Counts how often a future yields, which is once per blocking-pool hop.
struct PendingCount<F> {
    inner: Pin<Box<F>>,
    pending: usize,
}

impl<F: Future> Future for PendingCount<F> {
    type Output = (F::Output, usize);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.inner.as_mut().poll(cx) {
            Poll::Ready(output) => Poll::Ready((output, this.pending)),
            Poll::Pending => {
                this.pending += 1;
                Poll::Pending
            }
        }
    }
}

async fn count_pending<F: Future>(future: F) -> (F::Output, usize) {
    PendingCount {
        inner: Box::pin(future),
        pending: 0,
    }
    .await
}

fn populated_directory(count: usize) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..count {
        std::fs::write(dir.path().join(format!("entry-{index:04}.dat")), b"entry").unwrap();
    }
    dir
}

/// A blocking task completes with one wake, so more yields than this mean the
/// page took several blocking-pool hops.
const ONE_BLOCKING_TASK: usize = 1;

#[tokio::test]
async fn test_local_listing_batches_take_one_blocking_hop() {
    let fs = LocalFs::new();
    let dir = populated_directory(200);

    for detail in [ListingDetail::Fast, ListingDetail::Metadata] {
        let mut stream = fs
            .open_listing(dir.path(), options(detail), &ProviderCx::none())
            .await
            .unwrap()
            .expect("the local provider should expose a listing stream");
        let (batch, pending) = count_pending(stream.next_batch(200, &ProviderCx::none())).await;
        assert_eq!(batch.unwrap().entries.len(), 200);
        assert!(
            pending <= ONE_BLOCKING_TASK,
            "{detail:?} stream batch yielded {pending} times"
        );

        let request = DirectoryPageRequest {
            listing: options(detail),
            limit: 150,
            cursor: None,
        };
        let (page, pending) =
            count_pending(fs.list_page(dir.path(), request, &ProviderCx::none())).await;
        assert_eq!(page.unwrap().entries.len(), 150);
        assert!(
            pending <= ONE_BLOCKING_TASK,
            "{detail:?} offset page yielded {pending} times"
        );

        let (rows, pending) =
            count_pending(fs.list_with_options(dir.path(), options(detail), &ProviderCx::none()))
                .await;
        assert_eq!(rows.unwrap().len(), 200);
        assert!(
            pending <= ONE_BLOCKING_TASK,
            "{detail:?} full listing yielded {pending} times"
        );
    }
}

#[test]
fn test_local_listing_batch_stops_on_cancellation_inside_the_blocking_read() {
    let dir = populated_directory(4);
    let cancel = CancelSignal::new();
    cancel.cancel();
    let stop = ListingStop::new(&ProviderCx::with_cancel(&cancel));
    let mut read_dir = std::fs::read_dir(dir.path()).unwrap();

    let Err(error) = read_batch(&mut read_dir, dir.path(), ListingDetail::Metadata, 4, &stop)
    else {
        panic!("a cancelled batch should stop");
    };

    assert_eq!(error.code(), ErrorCode::Cancelled);
}

#[test]
fn test_local_listing_batch_stops_at_the_deadline_inside_the_blocking_read() {
    let dir = populated_directory(4);
    let stop = ListingStop::new(&ProviderCx::none().with_deadline(Instant::now()));
    let mut read_dir = std::fs::read_dir(dir.path()).unwrap();

    let Err(error) = read_batch(&mut read_dir, dir.path(), ListingDetail::Metadata, 4, &stop)
    else {
        panic!("a batch past its deadline should stop");
    };

    assert_eq!(error.code(), ErrorCode::TimedOut);
}

#[tokio::test]
async fn test_local_listing_stream_reports_a_deadline_that_passed_before_the_batch() {
    let fs = LocalFs::new();
    let dir = populated_directory(3);
    let mut stream = fs
        .open_listing(dir.path(), ListingOptions::metadata(), &ProviderCx::none())
        .await
        .unwrap()
        .expect("the local provider should expose a listing stream");

    let expired = ProviderCx::none().with_deadline(Instant::now());
    let error = stream
        .next_batch(3, &expired)
        .await
        .expect_err("an expired batch should fail");
    assert_eq!(error.code(), ErrorCode::TimedOut);

    // Nothing was read, so the walk is still whole.
    let batch = stream.next_batch(3, &ProviderCx::none()).await.unwrap();
    assert_eq!(batch.entries.len(), 3);
}

#[tokio::test]
async fn test_local_listing_stream_refuses_to_resume_after_an_abandoned_batch() {
    let fs = LocalFs::new();
    let dir = populated_directory(2_000);
    let mut stream = fs
        .open_listing(dir.path(), ListingOptions::metadata(), &ProviderCx::none())
        .await
        .unwrap()
        .expect("the local provider should expose a listing stream");

    // A zero timeout polls the batch once and drops it while its blocking read
    // holds the directory handle, as a cancelled caller's race would.
    let abandoned = tokio::time::timeout(
        std::time::Duration::ZERO,
        stream.next_batch(2_000, &ProviderCx::none()),
    )
    .await;
    assert!(abandoned.is_err(), "the batch should still be reading");

    let error = stream
        .next_batch(1, &ProviderCx::none())
        .await
        .expect_err("rows of the abandoned batch are gone, so resuming would skip them");
    assert_eq!(error.code(), ErrorCode::IoFailed);
}

#[tokio::test]
async fn test_local_full_listing_rejects_an_expired_deadline() {
    let fs = LocalFs::new();
    let dir = populated_directory(3);
    let expired = ProviderCx::none().with_deadline(Instant::now());

    let error = fs
        .list_with_options(dir.path(), ListingOptions::metadata(), &expired)
        .await
        .expect_err("an expired listing should fail");

    assert_eq!(error.code(), ErrorCode::TimedOut);
}
