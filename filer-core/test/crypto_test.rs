//! Tests for encryption services

use filer_core::services::crypto::cipher::{Cipher, CipherAlgorithm, EncryptedData};
use filer_core::services::crypto::key::{derive_key, generate_salt, KdfParams, KeyDerivation};

#[test]
fn test_aes256_gcm_encrypt_decrypt() {
    todo!()
}

#[test]
fn test_chacha20_poly1305_encrypt_decrypt() {
    todo!()
}

#[test]
fn test_xchacha20_poly1305_encrypt_decrypt() {
    todo!()
}

#[test]
fn test_wrong_key_fails() {
    todo!()
}

#[test]
fn test_corrupted_data_fails() {
    todo!()
}

#[test]
fn test_argon2id_derive_key() {
    todo!()
}

#[test]
fn test_scrypt_derive_key() {
    todo!()
}

#[test]
fn test_same_password_same_key() {
    todo!()
}

#[test]
fn test_different_salt_different_key() {
    todo!()
}

#[test]
fn test_generate_salt_length() {
    todo!()
}

#[test]
fn test_generate_salt_unique() {
    todo!()
}
