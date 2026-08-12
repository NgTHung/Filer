//! Session manager - manages multiple client sessions
//!
//! Each client (desktop window, web connection, etc.) gets its own session
//! with isolated navigation state and event channel.
//!
//! The SessionManager is owned by the CommandRouter, which calls it to:
//! 1. Create sessions (on Handshake)
//! 2. Validate sessions (before routing any command)
//! 3. Check authorization (via SessionPolicy)
//! 4. Destroy sessions (on DestroySession or disconnect)

use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::api::event_sink::{EventSendError, EventSink};
use crate::api::events::Event;
use crate::model::registry::NodeRegistry;
use crate::model::session::{AllowAll, SessionId, SessionPolicy};
use crate::modules::navigation::navigator::NavigatorState;

/// A client session with its own state, event channel, and policy
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,
    /// Navigation state for this session
    pub navigator: NavigatorState,
    /// Channel to send events to this client
    pub event_tx: EventSink,
    /// Authorization policy for this session
    pub policy: Box<dyn SessionPolicy>,
    /// Session metadata
    pub created_at: std::time::Instant,
}

impl Session {
    /// Create a new session with the default AllowAll policy (native desktop)
    pub fn new(id: SessionId, event_tx: EventSink, reg: NodeRegistry) -> Self {
        Self {
            id,
            navigator: NavigatorState::new(reg),
            event_tx,
            policy: Box::new(AllowAll),
            created_at: std::time::Instant::now(),
        }
    }

    /// Create a new session with a custom policy (web/remote clients)
    pub fn with_policy(
        id: SessionId,
        event_tx: EventSink,
        reg: NodeRegistry,
        policy: Box<dyn SessionPolicy>,
    ) -> Self {
        Self {
            id,
            navigator: NavigatorState::new(reg),
            event_tx,
            policy,
            created_at: std::time::Instant::now(),
        }
    }

    /// Send an event to this session's client
    pub fn send_event(&self, event: Event) -> Result<(), EventSendError> {
        self.event_tx.send(event)
    }
}

/// Manages multiple client sessions
///
/// Clone is cheap — both fields use Arc internally, so clones share the
/// same underlying session map and node registry.
#[derive(Debug, Clone)]
pub struct SessionManager {
    /// Active sessions by ID
    sessions: Arc<scc::HashMap<SessionId, Session, RandomState>>,
    /// Shared node registry
    registry: NodeRegistry,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(reg: NodeRegistry) -> Self {
        Self {
            sessions: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            registry: reg,
        }
    }

    /// Create a new session with default (AllowAll) policy.
    /// Used by native desktop clients and as default for Handshake.
    pub fn create_session<E: Into<EventSink>>(&self, event_tx: E) -> SessionId {
        let id = SessionId::new();
        let session = Session::new(id, event_tx.into(), self.registry.clone());
        let _ = self.sessions.insert_sync(id, session);
        id
    }

    /// Create a new session with a custom policy.
    /// Used by web/remote transport layers after authenticating the client.
    pub fn create_session_with_policy<E: Into<EventSink>>(
        &self,
        event_tx: E,
        policy: Box<dyn SessionPolicy>,
    ) -> SessionId {
        let id = SessionId::new();
        let session = Session::with_policy(id, event_tx.into(), self.registry.clone(), policy);
        let _ = self.sessions.insert_sync(id, session);
        id
    }

    /// Check whether a session exists (is valid)
    pub fn exists(&self, id: SessionId) -> bool {
        self.sessions.contains_sync(&id)
    }

    /// Check whether an operation is allowed for a session on a given path.
    /// Returns `false` if session does not exist.
    pub fn is_allowed(
        &self,
        id: SessionId,
        operation: crate::model::session::Operation,
        path: &std::path::Path,
    ) -> bool {
        self.sessions
            .read_sync(&id, |_, s| s.policy.is_allowed(operation, path))
            .unwrap_or(false)
    }

    /// Remove a session (client disconnected or DestroySession received)
    pub fn remove(&self, id: SessionId) -> bool {
        self.sessions.remove_sync(&id).is_some()
    }

    /// Get number of active sessions
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Broadcast an event to all sessions
    pub fn broadcast(&self, event: Event) {
        self.sessions.iter_sync(|_k, v| {
            let _ = v.send_event(event.clone());
            true
        });
    }

    /// Send event to a specific session.
    /// Returns Err if session not found or channel closed.
    pub fn send_to(&self, session: SessionId, event: Event) -> Result<(), SendError> {
        let result = self.sessions.read_sync(&session, |_, s| {
            s.send_event(event.clone())
                .map_err(|_| SendError::ChannelClosed)
        });
        match result {
            Some(inner) => inner,
            None => Err(SendError::SessionNotFound(session)),
        }
    }
}

/// Error when sending to a session
#[derive(Debug, Clone)]
pub enum SendError {
    /// Session not found
    SessionNotFound(SessionId),
    /// Channel closed
    ChannelClosed,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionNotFound(id) => write!(f, "session not found: {}", id),
            Self::ChannelClosed => write!(f, "channel closed"),
        }
    }
}

impl std::error::Error for SendError {}
