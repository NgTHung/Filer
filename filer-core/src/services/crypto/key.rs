use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::errors::CoreError;

/// Key derivation functions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDerivation {
    Argon2id,
    Scrypt,
    Pbkdf2,
}

/// Key derivation parameters
#[derive(Debug, Clone)]
pub struct KdfParams {
    pub algorithm: KeyDerivation,
    pub salt: Vec<u8>,
    pub iterations: u32,
    pub memory_kb: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            algorithm: KeyDerivation::Argon2id,
            salt: Vec::new(),
            iterations: 3,
            memory_kb: 65536,
            parallelism: 4,
        }
    }
}

/// Derive key from password
pub fn derive_key(password: &[u8], params: &KdfParams, key_len: usize) -> Result<Vec<u8>, CoreError> {
    todo!()
}

/// Generate random salt
pub fn generate_salt(len: usize) -> Vec<u8> {
    todo!()
}

/// Key storage
pub struct KeyStore {
    keys: Arc<scc::HashMap<String, Vec<u8>, RandomState>>,
}

impl KeyStore {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    /// Store key with identifier
    pub fn store(&mut self, id: &str, key: Vec<u8>) {
        let _ = self.keys.insert_sync(id.to_string(), key);
    }

    /// Get key by identifier
    pub fn get(&self, id: &str) -> Option<Vec<u8>>  {
        self.keys.read_sync(id, |_,v: &Vec::<u8>| v.clone())
    }

    /// Remove key
    pub fn remove(&mut self, id: &str) -> Option<Vec<u8>> {
        self.keys.remove_sync(id).map(|v| v.1)
    }

    /// Clear all keys (secure wipe)
    pub fn clear(&mut self) {
        self.keys.iter_mut_sync(|mut k| {
            k.1.clear();
            true
        });
        self.keys.clear_sync();
    }

    /// Load keystore from encrypted file
    pub async fn load(path: &std::path::Path, password: &[u8]) -> Result<Self, CoreError> {
        todo!()
    }

    /// Save keystore to encrypted file
    pub async fn save(&self, path: &std::path::Path, password: &[u8]) -> Result<(), CoreError> {
        todo!()
    }
}

impl Default for KeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeyStore {
    fn drop(&mut self) {
        self.clear();
    }
}
