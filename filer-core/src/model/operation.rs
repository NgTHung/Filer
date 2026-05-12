use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Unique identifier for a file operation flow.
///
/// Operation IDs are runtime-local correlation tokens. They are monotonic to
/// keep logs, tests, progress updates, and completion events easy to reason
/// about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub u64);

impl OperationId {
    /// Generate a new unique operation ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Default operation ID for compatibility placeholders.
    pub const DEFAULT: OperationId = OperationId(0);
}

impl Default for OperationId {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation:{}", self.0)
    }
}
