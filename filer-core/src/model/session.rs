//! Session management for multi-client support
//!
//! Each connected client (desktop window, web browser, etc.) gets its own session
//! with isolated navigation state.
//!
//! # Session Policy
//!
//! Each session has an associated `SessionPolicy` that controls what operations
//! are permitted. This separates authentication (a transport concern) from
//! authorization (a core concern):
//!
//! - **Native desktop**: Uses `AllowAll` — the OS user's filesystem permissions apply.
//! - **Web client**: Transport layer (WebSocket handshake) authenticates the user,
//!   then attaches a policy that may restrict accessible paths or operations.
//!
//! Authentication happens OUTSIDE filer-core (HTTP headers, tokens, TLS certs).
//! Authorization happens INSIDE filer-core via `SessionPolicy`.

use std::path::Path;
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

/// Operations that can be gated by policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Read directory listings, metadata, previews
    Read,
    /// Create, copy, move, rename, delete files
    Write,
    /// Execute/open files
    Execute,
    /// Watch directories for changes
    Watch,
    /// Search within paths
    Search,
}

/// Policy controlling what a session is allowed to do.
///
/// The transport layer creates the appropriate policy when establishing a session:
/// - Native app → `AllowAll` (trusts OS permissions)
/// - Web app → `RestrictedPolicy` with path and operation constraints
///
/// This trait is object-safe so policies can be stored as `Box<dyn SessionPolicy>`.
pub trait SessionPolicy: Send + Sync + std::fmt::Debug {
    /// Check whether the given operation is allowed on the given path.
    /// Returns `true` if permitted, `false` if denied.
    fn is_allowed(&self, operation: Operation, path: &Path) -> bool;

    /// Human-readable policy name (for logging/debugging)
    fn name(&self) -> &'static str;
}

/// Default policy for native desktop — everything is allowed.
/// Filesystem-level permissions are enforced by the OS.
#[derive(Debug, Clone)]
pub struct AllowAll;

impl SessionPolicy for AllowAll {
    fn is_allowed(&self, _operation: Operation, _path: &Path) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "allow-all"
    }
}

/// Restrictive policy for web/remote clients.
///
/// Constrains sessions to specific root paths and operation sets.
/// Created by the transport/auth layer after validating credentials.
#[derive(Debug, Clone)]
pub struct RestrictedPolicy {
    /// Paths this session may access (read/write scoped under these roots)
    pub allowed_roots: Vec<std::path::PathBuf>,
    /// Operations this session may perform
    pub allowed_ops: Vec<Operation>,
    /// Human label (e.g. username or role)
    pub label: String,
}

impl SessionPolicy for RestrictedPolicy {
    fn is_allowed(&self, operation: Operation, path: &Path) -> bool {
        // Check operation is in the allowed set
        if !self.allowed_ops.contains(&operation) {
            return false;
        }
        // Check path is under an allowed root
        self.allowed_roots.iter().any(|root| path.starts_with(root))
    }

    fn name(&self) -> &'static str {
        "restricted"
    }
}
