//! # Provider Secrets
//!
//! Provides a small wrapper for runtime provider credentials. Provider configs
//! may keep credentials in memory while a session runs, but Debug output must
//! never print those values because tracing can capture derived Debug strings.
//!
//! ```
//! use std::fmt;
//!
//! const REDACTED_SECRET: &str = "<redacted>";
//!
//! struct ProviderSecret(String);
//!
//! impl ProviderSecret {
//!     fn new(value: impl Into<String>) -> Self {
//!         Self(value.into())
//!     }
//! }
//!
//! impl fmt::Debug for ProviderSecret {
//!     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         formatter.write_str(REDACTED_SECRET)
//!     }
//! }
//!
//! let secret = ProviderSecret::new("token");
//! assert_eq!(format!("{secret:?}"), "<redacted>");
//! ```

use std::fmt;

#[allow(dead_code)]
pub(crate) const REDACTED_SECRET: &str = "<redacted>";

#[derive(Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct ProviderSecret(String);

#[allow(dead_code)]
impl ProviderSecret {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED_SECRET)
    }
}
