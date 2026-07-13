//! # Task Identity
//!
//! This module defines portable domain names and exact task identities. It
//! keeps parsing and validation shared by configuration, creation, and later
//! task-reference resolution.
//!
//! ```
//! use filer_task::identity::TaskIdentity;
//!
//! let identity = TaskIdentity::parse("default:WORK-001")?;
//! assert_eq!(identity.domain, "default");
//! assert_eq!(identity.id, "WORK-001");
//! assert_eq!(identity.to_string(), "default:WORK-001");
//! # Ok::<(), filer_task::identity::IdentityError>(())
//! ```

use std::{error::Error, fmt};

pub const DOMAIN_CONSTRAINT: &str = "must be a portable lowercase domain name";
pub const LOCAL_ID_CONSTRAINT: &str = "must use the PREFIX-NUMBER form";

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskIdentity {
    pub domain: String,
    pub id: String,
}

impl TaskIdentity {
    pub fn new(
        domain: impl Into<String>,
        local_id: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        let domain = domain.into();
        let local_id = local_id.into();
        if !is_valid_domain_name(&domain) {
            return Err(IdentityError::InvalidDomain(domain));
        }
        if !is_valid_local_id(&local_id) {
            return Err(IdentityError::InvalidLocalId(local_id));
        }
        Ok(Self {
            domain,
            id: local_id,
        })
    }

    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        let mut parts = value.split(':');
        let domain = parts.next().unwrap_or_default();
        let local_id = parts
            .next()
            .ok_or_else(|| IdentityError::InvalidFormat(value.to_string()))?;
        if parts.next().is_some() || domain.is_empty() || local_id.is_empty() {
            return Err(IdentityError::InvalidFormat(value.to_string()));
        }
        Self::new(domain, local_id)
    }
}

impl fmt::Display for TaskIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.domain, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    InvalidFormat(String),
    InvalidDomain(String),
    InvalidLocalId(String),
}

impl IdentityError {
    pub fn value(&self) -> &str {
        match self {
            Self::InvalidFormat(value)
            | Self::InvalidDomain(value)
            | Self::InvalidLocalId(value) => value,
        }
    }

    pub fn constraint(&self) -> &'static str {
        match self {
            Self::InvalidFormat(_) => "must use the domain:LOCAL-ID form",
            Self::InvalidDomain(_) => DOMAIN_CONSTRAINT,
            Self::InvalidLocalId(_) => LOCAL_ID_CONSTRAINT,
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid task identity {:?}: {}",
            self.value(),
            self.constraint()
        )
    }
}

impl Error for IdentityError {}

pub fn is_valid_domain_name(value: &str) -> bool {
    valid_hyphen_name(value, 64, true) && !is_windows_device_name(value)
}

pub fn is_valid_local_id(value: &str) -> bool {
    let Some((prefix, number)) = value.split_once('-') else {
        return false;
    };
    (1..=32).contains(&prefix.len())
        && prefix
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_uppercase)
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn valid_hyphen_name(value: &str, max: usize, starts_with_letter: bool) -> bool {
    let bytes = value.as_bytes();
    if !(1..=max).contains(&bytes.len()) {
        return false;
    }
    let first_valid = if starts_with_letter {
        bytes[0].is_ascii_lowercase()
    } else {
        bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()
    };
    first_valid
        && (bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

pub(crate) fn is_windows_device_name(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(
            upper.as_bytes(),
            [b'C', b'O', b'M', b'1'..=b'9'] | [b'L', b'P', b'T', b'1'..=b'9']
        )
}
