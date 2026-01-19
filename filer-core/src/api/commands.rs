use std::path::PathBuf;

use crate::model::node::NodeId;
use crate::PreviewOptions;

/// Commands from UI to Core
/// Uses NodeId for efficiency (8 bytes vs PathBuf's heap allocation)
/// Core resolves NodeId -> PathBuf via NodeRegistry
#[derive(Debug, Clone)]
pub enum Command {
    /// Navigate to path (initial navigation uses PathBuf)
    Navigate(PathBuf),
    
    /// Navigate to a node by ID (after initial load)
    NavigateToNode(NodeId),
    
    /// Go up one directory
    NavigateUp,
    
    /// Refresh current directory
    Refresh,
    
    /// Search for files
    Search {
        query: String,
        root: NodeId,
    },
    
    /// Cancel current operation
    Cancel,
    
    /// Load preview for a node
    LoadPreview {
        id: NodeId,
        options: Option<PreviewOptions>,
    },
    
    /// Cancel preview generation
    CancelPreview(NodeId),
    
    /// Copy nodes to destination
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
    },
    
    /// Move nodes to destination
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
    },
    
    /// Delete nodes
    Delete {
        nodes: Vec<NodeId>,
        trash: bool,
    },
    
    /// Rename a node
    Rename {
        node: NodeId,
        new_name: String,
    },
    
    /// Create folder in parent
    CreateFolder {
        parent: NodeId,
        name: String,
    },
    
    /// Create file in parent
    CreateFile {
        parent: NodeId,
        name: String,
    },
    
    /// Load basic metadata
    LoadMetadata(NodeId),
    
    /// Load extended metadata (EXIF, ID3, etc.)
    LoadExtendedMetadata(NodeId),
    
    /// Watch a directory for changes
    Watch(NodeId),
    
    /// Stop watching a directory
    Unwatch(NodeId),
}
