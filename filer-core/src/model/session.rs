//! Session management for multi-client support
//!
//! Each connected client (desktop window, web browser, etc.) gets its own session
//! with isolated navigation state.

use std::sync::atomic::{AtomicU64, Ordering};

/// Unique session identifier
///
/// Each client connection gets a unique SessionId. Commands and events
/// are tagged with SessionId to route to the correct client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

impl SessionId {
    /// Generate a new unique session ID
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Default session for single-client mode
    pub const DEFAULT: SessionId = SessionId(0);
}

impl Default for SessionId {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session:{}", self.0)
    }
}

