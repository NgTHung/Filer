//! Session manager - manages multiple client sessions
//!
//! Each client (desktop window, web connection, etc.) gets its own session
//! with isolated navigation state and event channel.

use std::collections::HashMap;

use flume::Sender;

use crate::api::events::Event;
use crate::model::session::SessionId;
use crate::actors::navigator::NavigatorState;

/// A client session with its own state and event channel
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,
    /// Navigation state for this session
    pub navigator: NavigatorState,
    /// Channel to send events to this client
    pub event_tx: Sender<Event>,
    /// Session metadata
    pub created_at: std::time::Instant,
}

impl Session {
    /// Create a new session
    pub fn new(id: SessionId, event_tx: Sender<Event>) -> Self {
        Self {
            id,
            navigator: NavigatorState::default(),
            event_tx,
            created_at: std::time::Instant::now(),
        }
    }

    /// Send an event to this session's client
    pub fn send_event(&self, event: Event) -> Result<(), flume::SendError<Event>> {
        self.event_tx.send(event)
    }
}

/// Manages multiple client sessions
#[derive(Debug, Default)]
pub struct SessionManager {
    /// Active sessions by ID
    sessions: HashMap<SessionId, Session>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new session for a client
    pub fn create_session(&mut self, event_tx: Sender<Event>) -> SessionId {
        todo!()
    }

    /// Get a session by ID
    pub fn get(&self, id: SessionId) -> Option<&Session> {
        todo!()
    }

    /// Get a mutable session by ID
    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        todo!()
    }

    /// Remove a session (client disconnected)
    pub fn remove(&mut self, id: SessionId) -> Option<Session> {
        todo!()
    }

    /// Check if session exists
    pub fn exists(&self, id: SessionId) -> bool {
        todo!()
    }

    /// Get number of active sessions
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Get all session IDs
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    /// Broadcast an event to all sessions
    pub fn broadcast(&self, event: Event) {
        todo!()
    }

    /// Send event to specific session
    pub fn send_to(&self, session: SessionId, event: Event) -> Result<(), SendError> {
        todo!()
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

