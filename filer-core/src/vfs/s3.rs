use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};
use crate::vfs::remote::{RemoteConfig, RemoteProvider};

/// S3 configuration
#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub session_token: Option<String>,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket: String::new(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: None,
            secret_key: None,
            session_token: None,
        }
    }
}

/// S3 filesystem provider
pub struct S3Fs {
    config: S3Config,
    connected: bool,
}

impl S3Fs {
    pub fn new(config: S3Config) -> Self {
        Self {
            config,
            connected: false,
        }
    }

    /// List objects with prefix
    async fn list_objects(&self, prefix: &str) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }

    /// Get object
    async fn get_object(&self, key: &str) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    /// Put object
    async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), CoreError> {
        todo!()
    }

    /// Delete object
    async fn delete_object(&self, key: &str) -> Result<(), CoreError> {
        todo!()
    }
}

#[async_trait]
impl FsProvider for S3Fs {
    fn scheme(&self) -> &'static str {
        "s3"
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
impl RemoteProvider for S3Fs {
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
