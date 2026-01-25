use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};
use crate::vfs::remote::{RemoteConfig, RemoteProvider};

/// WebDAV configuration
#[derive(Debug, Clone)]
pub struct WebDavConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub bearer_token: Option<String>,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: None,
            password: None,
            bearer_token: None,
        }
    }
}

/// WebDAV filesystem provider
pub struct WebDavFs {
    config: WebDavConfig,
    connected: bool,
}

impl WebDavFs {
    pub fn new(config: WebDavConfig) -> Self {
        Self {
            config,
            connected: false,
        }
    }

    /// PROPFIND request
    async fn propfind(&self, path: &str, depth: u8) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }

    /// GET request
    async fn get(&self, path: &str) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    /// PUT request
    async fn put(&self, path: &str, data: &[u8]) -> Result<(), CoreError> {
        todo!()
    }

    /// DELETE request
    async fn delete(&self, path: &str) -> Result<(), CoreError> {
        todo!()
    }

    /// MKCOL request (create directory)
    async fn mkcol(&self, path: &str) -> Result<(), CoreError> {
        todo!()
    }

    /// MOVE request
    async fn move_file(&self, from: &str, to: &str) -> Result<(), CoreError> {
        todo!()
    }

    /// COPY request
    async fn copy(&self, from: &str, to: &str) -> Result<(), CoreError> {
        todo!()
    }
}

#[async_trait]
impl FsProvider for WebDavFs {
    fn scheme(&self) -> &'static str {
        "webdav"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: true,
            watch: false,
            search: false,
        }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    async fn exists(&self, path: &Path) -> Result<bool, CoreError> {
        todo!()
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        todo!()
    }
}

#[async_trait]
impl RemoteProvider for WebDavFs {
    async fn connect(&mut self) -> Result<(), CoreError> {
        todo!()
    }

    async fn disconnect(&mut self) -> Result<(), CoreError> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}
