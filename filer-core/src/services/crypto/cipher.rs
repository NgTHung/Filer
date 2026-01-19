use crate::errors::CoreError;

/// Supported encryption algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherAlgorithm {
    Aes256Gcm,
    ChaCha20Poly1305,
    XChaCha20Poly1305,
}

/// Encrypted data with metadata
#[derive(Debug, Clone)]
pub struct EncryptedData {
    pub algorithm: CipherAlgorithm,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub tag: Vec<u8>,
}

/// Cipher for encryption/decryption
pub struct Cipher {
    algorithm: CipherAlgorithm,
    key: Vec<u8>,
}

impl Cipher {
    /// Create new cipher with key
    pub fn new(algorithm: CipherAlgorithm, key: Vec<u8>) -> Result<Self, CoreError> {
        todo!()
    }

    /// Encrypt data
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CoreError> {
        todo!()
    }

    /// Decrypt data
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    /// Encrypt file in-place or to destination
    pub async fn encrypt_file(&self, src: &std::path::Path, dst: Option<&std::path::Path>) -> Result<(), CoreError> {
        todo!()
    }

    /// Decrypt file in-place or to destination
    pub async fn decrypt_file(&self, src: &std::path::Path, dst: Option<&std::path::Path>) -> Result<(), CoreError> {
        todo!()
    }

    /// Encrypt stream
    pub fn encrypt_stream<R: std::io::Read, W: std::io::Write>(
        &self,
        reader: R,
        writer: W,
    ) -> Result<(), CoreError> {
        todo!()
    }

    /// Decrypt stream
    pub fn decrypt_stream<R: std::io::Read, W: std::io::Write>(
        &self,
        reader: R,
        writer: W,
    ) -> Result<(), CoreError> {
        todo!()
    }
}
