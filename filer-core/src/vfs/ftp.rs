use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};
use crate::vfs::remote::{RemoteConfig, RemoteProvider};

/// FTP/SFTP configuration
#[derive(Debug, Clone)]
pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub secure: bool,           // FTPS
    pub use_sftp: bool,         // SFTP instead of FTP
    pub private_key: Option<String>,
    pub passive_mode: bool,
}

impl Default for FtpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 21,
            username: None,
            password: None,
            secure: false,
            use_sftp: false,
            private_key: None,
            passive_mode: true,
        }
    }
}

/// FTP/SFTP filesystem provider
pub struct FtpFs {
    config: FtpConfig,
    connected: bool,
}

impl FtpFs {
    pub fn new(config: FtpConfig) -> Self {
        Self {
            config,
            connected: false,
        }
    }

    /// List directory
    async fn list_dir(&self, path: &str) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }

    /// Download file
    async fn download(&self, path: &str) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    /// Upload file
    async fn upload(&self, path: &str, data: &[u8]) -> Result<(), CoreError> {
        todo!()
    }

    /// Delete file
    async fn delete_file(&self, path: &str) -> Result<(), CoreError> {
        todo!()
    }

    /// Create directory
    async fn mkdir(&self, path: &str) -> Result<(), CoreError> {
        todo!()
    }

    /// Remove directory
    async fn rmdir(&self, path: &str) -> Result<(), CoreError> {
        todo!()
    }

    /// Rename/move
    async fn rename(&self, from: &str, to: &str) -> Result<(), CoreError> {
        todo!()
    }
}

#[async_trait]
impl FsProvider for FtpFs {
    fn scheme(&self) -> &'static str {
        if self.config.use_sftp {
            "sftp"
        } else {
            "ftp"
        }
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
impl RemoteProvider for FtpFs {
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
