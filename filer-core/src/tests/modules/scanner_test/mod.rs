use crate::errors::CoreError;
use crate::model::directory::{
    DirectoryCursor, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
use crate::model::location::{Location, LocationRef};
use crate::model::node::{NodeEntry, NodeKind, NodeMeta};
use crate::tests::fixtures::{local_file_node, local_node_entry};
use crate::vfs::listing_stream::{DirectoryStream, ListingBatch};
use crate::vfs::provider::{Capabilities, FsProvider, ListingOptions, ProviderPaging};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

fn make_file(name: &str, path: &str, size: u64, hidden: bool) -> NodeEntry {
    let path = PathBuf::from(format!("{path}/{name}"));
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_string);
    local_file_node(
        path,
        name,
        NodeKind::File { extension },
        size,
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        NodeMeta {
            hidden,
            ..NodeMeta::default()
        },
    )
}

fn location_ref(path: impl Into<PathBuf>) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

fn _make_file_with_ext(name: &str, path: &str, ext: Option<&str>, size: u64) -> NodeEntry {
    local_file_node(
        PathBuf::from(format!("{path}/{name}")),
        name,
        NodeKind::File {
            extension: ext.map(|s| s.to_string()),
        },
        size,
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        NodeMeta::default(),
    )
}

fn _make_dir(name: &str, full_path: &str, hidden: bool) -> NodeEntry {
    local_file_node(
        PathBuf::from(format!("{full_path}/{name}")),
        name,
        NodeKind::Directory {
            children_count: None,
        },
        0,
        Some(SystemTime::UNIX_EPOCH),
        NodeMeta {
            hidden,
            ..NodeMeta::default()
        },
    )
}

/// Mock filesystem provider for testing Scanner behavior.
/// Scanner tests use native entries throughout the provider boundary.
#[derive(Clone)]
struct MockProvider {
    files: Arc<Mutex<Vec<NodeEntry>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    page_calls: Arc<Mutex<Vec<(PathBuf, DirectoryPageRequest)>>>,
    list_options: Arc<Mutex<Vec<ListingOptions>>>,
    should_fail: Arc<Mutex<bool>>,
    delay_ms: Arc<Mutex<u64>>,
    native_paging: bool,
    streaming: bool,
    stream_stats: Arc<Mutex<StreamStats>>,
}

/// What a streaming walk has cost so far, so a test can prove a page did not
/// pay for the whole directory.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct StreamStats {
    batch_calls: usize,
    rows_yielded: usize,
    reached_end: bool,
}

/// A resumable walk over the mock's entries, positioned by the stream itself so
/// a continuation cannot depend on an offset into a mutated list.
struct MockListingStream {
    files: Arc<Mutex<Vec<NodeEntry>>>,
    stats: Arc<Mutex<StreamStats>>,
    position: usize,
}

#[async_trait]
impl DirectoryStream for MockListingStream {
    async fn next_batch(
        &mut self,
        max: usize,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<ListingBatch, CoreError> {
        if cx.is_cancelled() {
            return Err(CoreError::cancelled());
        }
        let files = self.files.lock().unwrap();
        let end = (self.position + max).min(files.len());
        let entries: Vec<NodeEntry> = files[self.position.min(files.len())..end]
            .iter()
            .cloned()
            .map(local_node_entry)
            .collect();
        self.position = end;
        let reached_end = end >= files.len();
        drop(files);

        let mut stats = self.stats.lock().unwrap();
        stats.batch_calls += 1;
        stats.rows_yielded += entries.len();
        stats.reached_end = reached_end;
        drop(stats);

        Ok(if reached_end {
            ListingBatch::final_batch(entries)
        } else {
            ListingBatch::partial(entries)
        })
    }
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            page_calls: Arc::new(Mutex::new(Vec::new())),
            list_options: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
            delay_ms: Arc::new(Mutex::new(0)),
            native_paging: true,
            streaming: false,
            stream_stats: Arc::new(Mutex::new(StreamStats::default())),
        }
    }

    fn fallback() -> Self {
        Self {
            native_paging: false,
            ..Self::new()
        }
    }

    /// A provider that exposes a resumable walk, as the local provider does.
    fn streaming() -> Self {
        Self {
            streaming: true,
            ..Self::new()
        }
    }

    fn stream_stats(&self) -> StreamStats {
        *self.stream_stats.lock().unwrap()
    }

    fn add_file(&self, node: NodeEntry) {
        self.files.lock().unwrap().push(node);
    }

    fn insert_file(&self, index: usize, node: NodeEntry) {
        self.files.lock().unwrap().insert(index, node);
    }

    fn get_list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    fn get_page_calls(&self) -> Vec<(PathBuf, DirectoryPageRequest)> {
        self.page_calls.lock().unwrap().clone()
    }

    fn get_list_options(&self) -> Vec<ListingOptions> {
        self.list_options.lock().unwrap().clone()
    }

    fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }

    fn set_delay_ms(&self, delay_ms: u64) {
        *self.delay_ms.lock().unwrap() = delay_ms;
    }
}

#[async_trait]
impl FsProvider for MockProvider {
    fn scheme(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    fn paging(&self) -> ProviderPaging {
        if self.native_paging {
            ProviderPaging::Native
        } else {
            ProviderPaging::Fallback
        }
    }

    async fn open_listing(
        &self,
        path: &Path,
        _options: ListingOptions,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Option<Box<dyn DirectoryStream>>, CoreError> {
        if !self.streaming {
            return Ok(None);
        }
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::not_found(path.to_path_buf()));
        }
        Ok(Some(Box::new(MockListingStream {
            files: self.files.clone(),
            stats: self.stream_stats.clone(),
            position: 0,
        })))
    }

    async fn list(
        &self,
        path: &Path,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        self.list_with_options(path, ListingOptions::default(), cx)
            .await
    }

    async fn list_with_options(
        &self,
        path: &Path,
        options: ListingOptions,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        self.list_calls.lock().unwrap().push(path.to_path_buf());
        self.list_options.lock().unwrap().push(options);
        Ok(self
            .files
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(local_node_entry)
            .collect())
    }

    async fn list_page(
        &self,
        path: &Path,
        request: DirectoryPageRequest,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<DirectoryPageResult, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        self.page_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), request.clone()));
        let start = request
            .cursor
            .as_ref()
            .and_then(|cursor| cursor.0.parse::<usize>().ok())
            .unwrap_or(0);
        let files = self.files.lock().unwrap();
        let end = (start + request.limit).min(files.len());
        let entries: Vec<crate::NodeEntry> = files[start..end]
            .iter()
            .cloned()
            .map(local_node_entry)
            .collect();
        let next_cursor = (end < files.len()).then(|| DirectoryCursor(end.to_string()));
        let state = if let Some(cursor) = next_cursor {
            DirectoryPageState::partial(entries.len(), None, cursor)
        } else {
            DirectoryPageState::complete(entries.len(), None)
        };
        Ok(DirectoryPageResult { entries, state })
    }

    async fn read(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        _path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<crate::NodeEntry, CoreError> {
        Err(CoreError::not_found(PathBuf::from("test")))
    }
}

#[cfg(test)]
mod scanner_cache_tests {
    include!("scanner_cache_tests.rs");
    include!("scan_location_default_emits_directory_entry_page_loaded.rs");
    include!("sparse_filter_returns_complete_page_without_empty_intermediate_page.rs");
    include!("scanner_forwards_listing_options_to_provider.rs");
    include!("stale_scan_location_result_is_suppressed.rs");
}

// Scanner actor integration tests moved to filer-core/tests/scanner_integration_test.rs

include!("scanner_command_tests.rs");

include!("mock_provider_tests.rs");

include!("paging_model_tests.rs");

include!("paging_session_tests.rs");

#[cfg(test)]
mod streaming_paging_tests {
    include!("streaming_paging_tests.rs");
}
