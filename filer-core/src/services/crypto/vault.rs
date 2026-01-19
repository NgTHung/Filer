use std::path::{Path, PathBuf};
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};

use super::cipher::{Cipher, CipherAlgorithm};
use super::key::KeyStore;

/// Vault configuration
#[derive(Debug, Clone)]
pub struct VaultConfig {
    pub path: PathBuf,
    pub algorithm: CipherAlgorithm,
    pub filename_encryption: bool,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            algorithm: CipherAlgorithm::XChaCha20Poly1305,
            filename_encryption: true,
        }
    }
}

/// Encrypted vault - transparent encryption layer over another FsProvider
pub struct Vault {
    config: VaultConfig,
    inner: Box<dyn FsProvider>,
    cipher: Option<Cipher>,
    unlocked: bool,
}

impl Vault {
    pub fn new(config: VaultConfig, inner: Box<dyn FsProvider>) -> Self {
        Self {
            config,
            inner,
            cipher: None,
            unlocked: false,
        }
    }

    /// Create new vault with password
    pub async fn create(config: VaultConfig, inner: Box<dyn FsProvider>, password: &[u8]) -> Result<Self, CoreError> {
        todo!()
    }

    /// Open existing vault with password
    pub async fn open(config: VaultConfig, inner: Box<dyn FsProvider>, password: &[u8]) -> Result<Self, CoreError> {
        todo!()
    }

    /// Unlock vault with password
    pub fn unlock(&mut self, password: &[u8]) -> Result<(), CoreError> {
        todo!()
    }

    /// Lock vault (clear keys from memory)
    pub fn lock(&mut self) {
        self.cipher = None;
        self.unlocked = false;
    }

    /// Check if vault is unlocked
    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }

    /// Change vault password
    pub async fn change_password(&mut self, old_password: &[u8], new_password: &[u8]) -> Result<(), CoreError> {
        todo!()
    }

    /// Encrypt filename
    fn encrypt_name(&self, name: &str) -> Result<String, CoreError> {
        todo!()
    }

    /// Decrypt filename
    fn decrypt_name(&self, encrypted: &str) -> Result<String, CoreError> {
        todo!()
    }
}

#[async_trait]
impl FsProvider for Vault {
    fn scheme(&self) -> &'static str {
        "vault"
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
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

    async fn exists(&self, path: &Path) -> bool {
        todo!()
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        todo!()
    }
}
