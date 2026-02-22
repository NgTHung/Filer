use std::path::PathBuf;

use crate::model::node::NodeId;
use crate::pipeline::PipelineConfig;
use crate::PreviewOptions;
use crate::model::session::SessionId;

/// Commands from UI to Core
/// Uses NodeId for efficiency (8 bytes vs PathBuf's heap allocation)
/// Core resolves NodeId -> PathBuf via NodeRegistry
#[derive(Debug, Clone)]
pub enum Command {
    /// Navigate to path (initial navigation uses PathBuf)
    Navigate(PathBuf, SessionId),
    
    /// Navigate to a node by ID (after initial load)
    NavigateToNode(NodeId, SessionId),
    
    /// Go up one directory
    NavigateUp(SessionId),
    
    /// Refresh current directory
    Refresh(SessionId),
    
    /// Search for files
    Search {
        query: String,
        root: NodeId,
        session: SessionId
    },
    
    SearchPath {
        query: String,
        root: PathBuf,
        session: SessionId
    },
    
    /// Cancel current operation
    Cancel(SessionId),
    
    /// Load preview for a node
    LoadPreview {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId
    },
    
    /// Cancel preview generation
    CancelPreview(SessionId),
    
    /// Copy nodes to destination
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId
    },
    
    /// Move nodes to destination
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId
    },
    
    /// Delete nodes
    Delete {
        nodes: Vec<NodeId>,
        trash: bool,
        session: SessionId
    },
    
    /// Rename a node
    Rename {
        node: NodeId,
        new_name: String,
        session: SessionId
    },
    
    /// Create folder in parent
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId
    },
    
    /// Create file in parent
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId
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

    /// Cancel an active scan for this session
    CancelScan(SessionId),

    /// Watch a directory for changes
    Watch(NodeId,SessionId),
    
    /// Stop watching a directory
    Unwatch(NodeId),
    UnwatchSession(SessionId),

    Handshake,
    
    DestroySession(SessionId)
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
            | Command::UnwatchSession(s) => Some(*s),

            Command::Search { session, .. }
            | Command::SearchPath { session, .. }
            | Command::Scan { session, .. }
            | Command::ScanNode { session, .. }
            | Command::LoadPreview { session, .. }
            | Command::Copy { session, .. }
            | Command::Move { session, .. }
            | Command::Delete { session, .. }
            | Command::Rename { session, .. }
            | Command::CreateFolder { session, .. }
            | Command::CreateFile { session, .. } => Some(*session),
        }
    }
}
