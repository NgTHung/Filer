use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};

/// Remote filesystem connection config
#[derive(Debug, Clone)]
pub struct RemoteConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_secs: u64,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: None,
            username: None,
            password: None,
            timeout_secs: 30,
        }
    }
}

/// Trait for remote filesystem providers with connection management
#[async_trait]
pub trait RemoteProvider: FsProvider {
    /// Connect to remote server
    async fn connect(&mut self) -> Result<(), CoreError>;
    
    /// Disconnect from remote server
    async fn disconnect(&mut self) -> Result<(), CoreError>;
    
    /// Check if connected
    fn is_connected(&self) -> bool;
    
    /// Reconnect if disconnected
    async fn ensure_connected(&mut self) -> Result<(), CoreError> {
        if !self.is_connected() {
            self.connect().await?;
        }
        Ok(())
    }
}
