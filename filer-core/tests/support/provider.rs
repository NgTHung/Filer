//! # Memory provider fixtures
//!
//! Share ordinary listings and successful-call logs across suites while keeping
//! providers with paging or cancellation instrumentation beside their tests.
//!
//! ```
//! use filer_core::Capabilities;
//!
//! let capabilities = Capabilities { read: true, write: false, watch: false, search: false };
//! assert!(capabilities.read);
//! ```

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::core::{Capabilities, CoreError, FsProvider, NodeEntry};

/// Scanner assertions use native entries throughout the provider boundary.
#[derive(Clone)]
pub(crate) struct MemoryProvider {
    files: Arc<Mutex<Vec<NodeEntry>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MemoryProvider {
    pub(crate) fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub(crate) fn add_file(&self, node: NodeEntry) {
        self.files.lock().unwrap().push(node);
    }

    pub(crate) fn get_list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    pub(crate) fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }
}

#[async_trait]
impl FsProvider for MemoryProvider {
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

    async fn list(
        &self,
        path: &Path,
        _cx: &super::core::ProviderCx<'_>,
    ) -> Result<Vec<super::core::NodeEntry>, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::not_found(path.to_path_buf()));
        }
        self.list_calls.lock().unwrap().push(path.to_path_buf());
        Ok(self.files.lock().unwrap().clone())
    }

    async fn read(
        &self,
        _path: &Path,
        _cx: &super::core::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &super::core::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(
        &self,
        _path: &Path,
        _cx: &super::core::ProviderCx<'_>,
    ) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        _path: &Path,
        _cx: &super::core::ProviderCx<'_>,
    ) -> Result<super::core::NodeEntry, CoreError> {
        Err(CoreError::not_found(PathBuf::from("test")))
    }
}
