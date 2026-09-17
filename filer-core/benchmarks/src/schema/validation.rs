//! # Schema validation helpers
//!
//! These small helpers centralize closed syntax checks shared by request and
//! event conversion.

use crate::{ErrorCode, ProtocolError};

pub(super) const PROTOCOL_VERSION: u64 = 1;
const DIGEST_PREFIX: &str = "sha256:";

pub(super) fn validate_identifier(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.len() > 128 {
        return Err(schema_error("identifier length is outside 1..=128"));
    }
    let mut bytes = value.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(schema_error(
            "identifier does not match the protocol syntax",
        ));
    }
    Ok(())
}

pub(super) fn validate_digest(value: &str) -> Result<(), ProtocolError> {
    if value.len() != DIGEST_PREFIX.len() + 64
        || !value.starts_with(DIGEST_PREFIX)
        || !value[DIGEST_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
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
