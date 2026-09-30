use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Unique identifier for an async request/response flow.
///
/// Request IDs are runtime-local correlation tokens. They are monotonic to keep
/// logs, tests, and stale-result checks easy to reason about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub u64);

impl RequestId {
    /// Generate a new unique request ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Default request ID for compatibility placeholders.
    pub const DEFAULT: RequestId = RequestId(0);
}

impl Default for RequestId {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request:{}", self.0)
    }
}
