//! Tests for encryption services
#[cfg(feature = "crypto")]
use crate::services::crypto::cipher::{Cipher, CipherAlgorithm, EncryptedData};
#[cfg(feature = "crypto")]
use crate::services::crypto::key::{derive_key, generate_salt, KdfParams, KeyDerivation};

#[test]
#[cfg(feature = "crypto")]
fn test_aes256_gcm_encrypt_decrypt() {
    todo!()
}

#[test]
#[cfg(feature = "crypto")]
fn test_chacha20_poly1305_encrypt_decrypt() {
    todo!()
}

#[test]
#[cfg(feature = "crypto")]
fn test_xchacha20_poly1305_encrypt_decrypt() {
    todo!()
}

#[test]
#[cfg(feature = "crypto")]
fn test_wrong_key_fails() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_corrupted_data_fails() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_argon2id_derive_key() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_scrypt_derive_key() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_same_password_same_key() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_different_salt_different_key() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_generate_salt_length() {
    todo!()
}

#[cfg(feature = "crypto")]
#[test]
fn test_generate_salt_unique() {
    todo!()
}
