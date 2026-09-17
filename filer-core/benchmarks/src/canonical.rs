//! # Canonical rows and digests
//!
//! This module is the single source for row projection and digest encoding.
//! Both trace validation and filesystem readback use it so a semantic value
//! cannot acquire two subtly different representations.

use sha2::{Digest, Sha256};

use crate::{Field, Kind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRow {
    pub identity: String,
    pub kind: Kind,
    pub size_bytes: Option<u64>,
    pub modified_unix_ns: i64,
}

impl CanonicalRow {
    pub fn new(
        identity: impl Into<String>,
        kind: Kind,
        size_bytes: Option<u64>,
        modified_unix_ns: i64,
    ) -> Self {
        Self {
            identity: identity.into(),
            kind,
            size_bytes,
            modified_unix_ns,
        }
    }

    pub fn value(&self, field: Field) -> String {
        match field {
            Field::Identity => self.identity.clone(),
            Field::Kind => match self.kind {
                Kind::File => "file".to_string(),
                Kind::Directory => "directory".to_string(),
            },
            Field::SizeBytes => self
                .size_bytes
                .map_or_else(|| "~".to_string(), |value| value.to_string()),
            Field::ModifiedUnixNs => self.modified_unix_ns.to_string(),
        }
    }
}

pub fn canonical_digest(scope: &str, fields: &[Field], rows: &[CanonicalRow]) -> String {
    let mut ordered = rows.to_vec();
    if matches!(scope, "membership" | "metadata") {
        ordered.sort_by(|left, right| left.identity.as_bytes().cmp(right.identity.as_bytes()));
    }
    let values = ordered
        .iter()
        .map(|row| {
            fields
                .iter()
                .map(|field| row.value(*field))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    digest_records(
        scope,
        fields.iter().map(|field| field.as_str()).collect(),
        values,
    )
}

pub(crate) fn digest_records(scope: &str, fields: Vec<&str>, rows: Vec<Vec<String>>) -> String {
    let mut hasher = Sha256::new();
    for token in [
        "filer-benchmark-digest-v1".to_string(),
        scope.to_string(),
        fields.len().to_string(),
    ] {
        write_token(&mut hasher, &token);
    }
    for field in fields {
        write_token(&mut hasher, field);
    }
    write_token(&mut hasher, &rows.len().to_string());
    for row in rows {
        for value in row {
            write_token(&mut hasher, &value);
        }
    }
    hex_digest(&hasher.finalize())
}

fn write_token(hasher: &mut Sha256, token: &str) {
    let length = token.len().to_string();
    hasher.update(length.as_bytes());
    hasher.update(b":");
    hasher.update(token.as_bytes());
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(7 + bytes.len() * 2);
    value.push_str("sha256:");
    for byte in bytes {
        value.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        value.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    value
}
