//! # Schema validation helpers
//!
//! These small helpers centralize closed syntax checks shared by request and
//! event conversion.

use crate::{ErrorCode, ProtocolError};

pub(super) const PROTOCOL_VERSION: u64 = 1;
const DIGEST_PREFIX: &str = "sha256:";

pub(crate) fn is_valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

pub(super) fn validate_identifier(value: &str) -> Result<(), ProtocolError> {
    if !is_valid_identifier(value) {
        return Err(schema_error(
            "identifier does not match the protocol syntax",
        ));
    }
    Ok(())
}

pub(crate) fn is_valid_digest(value: &str) -> bool {
    value.len() == DIGEST_PREFIX.len() + 64
        && value.starts_with(DIGEST_PREFIX)
        && value[DIGEST_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(super) fn validate_digest(value: &str) -> Result<(), ProtocolError> {
    if !is_valid_digest(value) {
        return Err(schema_error("digest must match sha256:[0-9a-f]{64}"));
    }
    Ok(())
}

pub(super) fn schema_error(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ErrorCode::InvalidSchema, message)
}

pub(super) fn malformed(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ErrorCode::MalformedJson, message)
}
