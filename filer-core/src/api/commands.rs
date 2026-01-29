use std::path::PathBuf;

use crate::model::node::NodeId;
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
    
    /// Cancel current operation
    Cancel(SessionId),
    
    /// Load preview for a node
    LoadPreview {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId
    },
    
    /// Cancel preview generation
    CancelPreview(NodeId, SessionId),
    
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
    
    /// Watch a directory for changes
    Watch(NodeId,SessionId),
    
    /// Stop watching a directory
    Unwatch(NodeId,SessionId),

    Handshake,
    
    DestroySession(SessionId)
}
