use std::path::PathBuf;
use std::sync::Arc;

use crate::PreviewOptions;
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;

/// Commands from UI to Core
/// Uses NodeId for efficiency (8 bytes vs PathBuf's heap allocation)
/// Core resolves NodeId -> PathBuf via NodeRegistry
#[derive(Clone)]
pub enum Command {
    /// Navigate to path (initial navigation uses PathBuf)
    Navigate(PathBuf, SessionId),

    /// Navigate to a node by ID (after initial load)
    NavigateToNode(NodeId, SessionId),

    /// Go up one directory
    NavigateUp(SessionId),

    /// Go back in history
    NavigateBack(SessionId),

    /// Go forward in history
    NavigateForward(SessionId),

    /// Refresh current directory
    Refresh(SessionId),

    /// Search for files
    Search {
        query: String,
        root: NodeId,
        session: SessionId,
    },

    SearchPath {
        query: String,
        root: PathBuf,
        session: SessionId,
    },

    /// Cancel current operation
    Cancel(SessionId),

    /// Load preview for a node
    LoadPreview {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
    },

    /// Cancel preview generation
    CancelPreview(SessionId),

    /// Copy nodes to destination
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
    },

    /// Move nodes to destination
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
    },

    /// Delete nodes
    Delete {
        nodes: Vec<NodeId>,
        trash: bool,
        session: SessionId,
    },

    /// Rename a node
    Rename {
        node: NodeId,
        new_name: String,
        session: SessionId,
    },

    /// Create folder in parent
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId,
    },

    /// Create file in parent
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId,
    },

    /// Load basic metadata
    LoadMetadata(NodeId, SessionId),

    /// Load extended metadata (EXIF, ID3, etc.)
    LoadExtendedMetadata(NodeId, SessionId),

    /// Scan a directory by path (initial scan, returns batched results)
    Scan {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
    },

    /// Scan a directory by NodeId (re-scan after navigation)
    ScanNode {
        node: NodeId,
        session: SessionId,
        pipeline: PipelineConfig,
    },

    /// Update the current navigation pipeline for future refreshes.
    SetPipeline {
        session: SessionId,
        config: PipelineConfig,
    },

    /// Cancel an active scan for this session
    CancelScan(SessionId),

    /// Watch a directory for changes
    Watch(NodeId, SessionId),

    /// Stop watching a directory
    Unwatch(NodeId),
    UnwatchSession(SessionId),

    Handshake,

    DestroySession(SessionId),

    /// Extension point for custom commands from modules/plugins.
    ///
    /// The router dispatches these to handlers registered by key.
    /// Use `Arc<dyn Any + Send + Sync>` for a clonable, type-erased payload.
    ///
    /// # Example
    /// ```ignore
    /// core.send(Command::Extension {
    ///     key: "git.status".into(),
    ///     payload: Arc::new(GitStatusRequest { repo: path }),
    ///     session: my_session,
    /// });
    /// ```
    Extension {
        key: String,
        payload: Arc<dyn std::any::Any + Send + Sync>,
        session: SessionId,
    },
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Extension { key, session, .. } => f
                .debug_struct("Extension")
                .field("key", key)
                .field("session", session)
                .finish(),
            other => write!(f, "Command::{}", other.key()),
        }
    }
}

impl Command {
    /// Extract the SessionId from any command variant.
    ///
    /// Returns `None` for commands that don't require session validation:
    /// - `Handshake` (creates a session)
    /// - `DestroySession` (tears down — safe to no-op if unknown)
    /// - `Unwatch` (operates on NodeId, not session-scoped)
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            Command::Handshake => None,
            Command::DestroySession(_) => None,
            Command::Unwatch(_) => None,

            Command::Navigate(_, s)
            | Command::NavigateToNode(_, s)
            | Command::NavigateUp(s)
            | Command::Refresh(s)
            | Command::Cancel(s)
            | Command::CancelPreview(s)
            | Command::CancelScan(s)
            | Command::LoadMetadata(_, s)
            | Command::LoadExtendedMetadata(_, s)
            | Command::Watch(_, s)
            | Command::UnwatchSession(s)
            | Command::NavigateBack(s)
            | Command::NavigateForward(s) => Some(*s),

            Command::Search { session, .. }
            | Command::SearchPath { session, .. }
            | Command::Scan { session, .. }
            | Command::ScanNode { session, .. }
            | Command::SetPipeline { session, .. }
            | Command::LoadPreview { session, .. }
            | Command::Copy { session, .. }
            | Command::Move { session, .. }
            | Command::Delete { session, .. }
            | Command::Rename { session, .. }
            | Command::CreateFolder { session, .. }
            | Command::CreateFile { session, .. }
            | Command::Extension { session, .. } => Some(*session),
        }
    }

    /// Get the dispatch key for this command.
    ///
    /// The [`CommandRouter`] uses this to look up the registered handler.
    /// Core command variants return static keys; [`Extension`](Command::Extension)
    /// returns the user-provided key.
    pub fn key(&self) -> &str {
        match self {
            Command::Navigate(..) => "navigate",
            Command::NavigateToNode(..) => "navigate.node",
            Command::NavigateUp(..) => "navigate.up",
            Command::NavigateBack(..) => "navigate.back",
            Command::NavigateForward(..) => "navigate.forward",
            Command::Refresh(..) => "navigate.refresh",
            Command::Search { .. } => "search",
            Command::SearchPath { .. } => "search.path",
            Command::Cancel(..) => "search.cancel",
            Command::LoadPreview { .. } => "preview.load",
            Command::CancelPreview(..) => "preview.cancel",
            Command::LoadMetadata(..) => "metadata.load",
            Command::LoadExtendedMetadata(..) => "metadata.extended",
            Command::Copy { .. } => "ops.copy",
            Command::Move { .. } => "ops.move",
            Command::Delete { .. } => "ops.delete",
            Command::Rename { .. } => "ops.rename",
            Command::CreateFolder { .. } => "ops.create_folder",
            Command::CreateFile { .. } => "ops.create_file",
            Command::Scan { .. } => "scan",
            Command::ScanNode { .. } => "scan.node",
            Command::SetPipeline { .. } => "navigate.pipeline",
            Command::CancelScan(..) => "scan.cancel",
            Command::Watch(..) => "watch",
            Command::Unwatch(..) => "watch.remove",
            Command::UnwatchSession(..) => "watch.session_remove",
            Command::Handshake => "session.handshake",
            Command::DestroySession(..) => "session.destroy",
            Command::Extension { key, .. } => key.as_str(),
        }
    }
}
