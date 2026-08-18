use crate::errors::CoreError;
use crate::model::directory::{
    DirectoryCursor, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
use crate::model::location::{Location, LocationRef};
use crate::tests::fixtures::local_node_entry;
// Provider-shaped FileNode rows still require NodeId fields; scanner behavior
// assertions use Location-native entries and events.
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider, ListingOptions, ProviderPaging};
use crate::{
    model::node::{NodeId, NodeKind, NodeMeta},
    utils,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

fn make_file(name: &str, path: &str, size: u64, hidden: bool) -> FileNode {
    let extension = utils::get_extension(PathBuf::from(name).as_path()).map(str::to_string);
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("{path}/{name}")),
        kind: NodeKind::File { extension },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    }
}

fn location_ref(path: impl Into<PathBuf>) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

fn _make_file_with_ext(name: &str, path: &str, ext: Option<&str>, size: u64) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("{path}/{name}")),
        kind: NodeKind::File {
            extension: ext.map(|s| s.to_string()),
        },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    }
}

fn _make_dir(name: &str, full_path: &str, hidden: bool) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64 + 1000),
        name: name.to_string(),
        path: PathBuf::from(format!("{full_path}/{name}")),
        kind: NodeKind::Directory {
            children_count: None,
        },
        size: 0,
        modified: Some(SystemTime::UNIX_EPOCH),
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    }
}

/// Mock filesystem provider for testing Scanner behavior.
/// FileNode values stay at the FsProvider boundary; assertions use native locations and entries.
#[derive(Clone)]
struct MockProvider {
    files: Arc<Mutex<Vec<FileNode>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    page_calls: Arc<Mutex<Vec<(PathBuf, DirectoryPageRequest)>>>,
    list_options: Arc<Mutex<Vec<ListingOptions>>>,
    should_fail: Arc<Mutex<bool>>,
    delay_ms: Arc<Mutex<u64>>,
    native_paging: bool,
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
        }
    }

    fn fallback() -> Self {
        Self {
            native_paging: false,
            ..Self::new()
        }
    }

    fn add_file(&self, node: FileNode) {
        self.files.lock().unwrap().push(node);
    }

    fn insert_file(&self, index: usize, node: FileNode) {
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
