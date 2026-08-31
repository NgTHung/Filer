//! Tests for resumable provider directory listing streams.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::errors::{CoreError, ErrorCode};
use crate::model::cancel::CancelSignal;
use crate::model::node::NodeEntry;
use crate::vfs::context::ProviderCx;
use crate::vfs::listing_stream::DirectoryStream;
use crate::vfs::local::LocalFs;
use crate::vfs::provider::{Capabilities, FsProvider, ListingOptions};
use tempfile::TempDir;

fn local_fs() -> (LocalFs, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let fs = LocalFs::new();
    (fs, dir)
}

fn write_entries(dir: &Path, names: &[&str]) {
    for name in names {
        std::fs::write(dir.join(name), b"entry").unwrap();
    }
}

async fn open_stream(
    fs: &LocalFs,
    dir: &Path,
    options: ListingOptions,
) -> Box<dyn DirectoryStream> {
    fs.open_listing(dir, options, &ProviderCx::none())
        .await
        .expect("opening a local listing stream should succeed")
        .expect("the local provider should expose a listing stream")
}

async fn drain(stream: &mut dyn DirectoryStream, batch: usize) -> Vec<String> {
    let mut names = Vec::new();
    loop {
        let page = stream
            .next_batch(batch, &ProviderCx::none())
            .await
            .expect("batch should read");
        names.extend(page.entries.into_iter().map(|entry| entry.name));
        if page.end_of_directory {
            return names;
        }
    }
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

#[tokio::test]
async fn test_local_listing_stream_yields_every_entry_once_across_batches() {
    let (fs, dir) = local_fs();
    write_entries(dir.path(), &["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"]);

    let mut stream = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    let names = sorted(drain(stream.as_mut(), 2).await);

    assert_eq!(names, ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"]);
}

#[tokio::test]
async fn test_local_listing_stream_resumes_from_a_stored_handle() {
    let (fs, dir) = local_fs();
    write_entries(dir.path(), &["a.txt", "b.txt", "c.txt", "d.txt"]);

    let mut stream = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    let first = stream
        .next_batch(2, &ProviderCx::none())
        .await
        .expect("first batch should read");
    assert_eq!(first.entries.len(), 2);

    // The paging session stores the handle between requests, so resumption must
    // not need an offset or a prior entry key.
    let mut stored = Some(stream);
    let mut resumed = stored.take().expect("stored stream should resume");
    let rest = drain(resumed.as_mut(), 2).await;

    let mut all: Vec<String> = first.entries.into_iter().map(|entry| entry.name).collect();
    all.extend(rest);
    assert_eq!(sorted(all), ["a.txt", "b.txt", "c.txt", "d.txt"]);
}

#[tokio::test]
async fn test_local_listing_stream_reports_end_of_directory_on_the_final_batch() {
    let (fs, dir) = local_fs();
    write_entries(dir.path(), &["a.txt", "b.txt"]);

    let mut stream = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    let page = stream
        .next_batch(8, &ProviderCx::none())
        .await
        .expect("batch should read");

    assert_eq!(page.entries.len(), 2);
    assert!(page.end_of_directory);
}

#[tokio::test]
async fn test_local_listing_stream_reports_end_of_directory_for_an_empty_directory() {
    let (fs, dir) = local_fs();

    let mut stream = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    let page = stream
        .next_batch(4, &ProviderCx::none())
        .await
        .expect("batch should read");

    assert!(page.entries.is_empty());
    assert!(page.end_of_directory);
}

#[tokio::test]
async fn test_local_listing_stream_fills_metadata_only_when_requested() {
    let (fs, dir) = local_fs();
    std::fs::write(dir.path().join("sized.txt"), b"0123456789").unwrap();

    let mut fast = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    let fast_entry = single_entry(fast.as_mut()).await;
    assert_eq!(fast_entry.size, 0);

    let mut detailed = open_stream(&fs, dir.path(), ListingOptions::metadata()).await;
    let detailed_entry = single_entry(detailed.as_mut()).await;
    assert_eq!(detailed_entry.size, 10);
}

async fn single_entry(stream: &mut dyn DirectoryStream) -> NodeEntry {
    stream
        .next_batch(4, &ProviderCx::none())
        .await
        .expect("batch should read")
        .entries
        .pop()
        .expect("directory should have one entry")
}

#[tokio::test]
async fn test_local_listing_stream_rejects_a_cancelled_context_between_batches() {
    let (fs, dir) = local_fs();
    write_entries(dir.path(), &["a.txt", "b.txt", "c.txt"]);

    let mut stream = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    stream
        .next_batch(1, &ProviderCx::none())
        .await
        .expect("first batch should read");

    let cancel = CancelSignal::new();
    cancel.cancel();
    let error = stream
        .next_batch(1, &ProviderCx::with_cancel(&cancel))
        .await
        .expect_err("a cancelled batch should fail");

    assert_eq!(error.code(), ErrorCode::Cancelled);
}

#[tokio::test]
async fn test_local_provider_rejects_opening_a_listing_stream_on_a_missing_directory() {
    let (fs, dir) = local_fs();
    let missing = dir.path().join("missing");

    let Err(error) = fs
        .open_listing(&missing, ListingOptions::fast(), &ProviderCx::none())
        .await
    else {
        panic!("a missing directory should fail to open");
    };

    assert_eq!(error.code(), ErrorCode::PathNotFound);
}

/// Counts descriptors in this process that point at `dir`, so a leaked handle is
/// visible without instrumenting production code. Each test uses its own
/// temporary directory, so parallel tests cannot disturb the count.
#[cfg(target_os = "linux")]
fn open_descriptors_for(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir("/proc/self/fd") else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| std::fs::read_link(entry.path()).is_ok_and(|target| target == dir))
        .count()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_local_listing_stream_releases_its_handle_on_drop() {
    let (fs, dir) = local_fs();
    write_entries(dir.path(), &["a.txt", "b.txt"]);
    let canonical = dir.path().canonicalize().unwrap();
    assert_eq!(open_descriptors_for(&canonical), 0);

    let mut stream = open_stream(&fs, dir.path(), ListingOptions::fast()).await;
    stream
        .next_batch(1, &ProviderCx::none())
        .await
        .expect("first batch should read");
    assert_eq!(open_descriptors_for(&canonical), 1);

    drop(stream);

    assert_eq!(open_descriptors_for(&canonical), 0);
}

/// A provider that takes every `FsProvider` default, including the absence of a
/// listing stream.
struct DefaultProvider;

#[async_trait]
impl FsProvider for DefaultProvider {
    fn scheme(&self) -> &'static str {
        "default"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    async fn list(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        Ok(Vec::new())
    }

    async fn read(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Ok(Vec::new())
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(Vec::new())
    }

    async fn exists(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(&self, path: &Path, _cx: &ProviderCx<'_>) -> Result<NodeEntry, CoreError> {
        Err(CoreError::not_found(PathBuf::from(path)))
    }
}

#[tokio::test]
async fn test_provider_without_native_paging_exposes_no_listing_stream() {
    let provider = DefaultProvider;

    let stream = provider
        .open_listing(
            Path::new("/tmp/default-provider"),
            ListingOptions::fast(),
            &ProviderCx::none(),
        )
        .await
        .expect("the default listing stream should not fail");

    assert!(stream.is_none());
}
