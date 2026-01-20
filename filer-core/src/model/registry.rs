use std::collections::HashMap;
use std::path::PathBuf;

use super::node::NodeId;

/// Registry that maps NodeId to PathBuf
/// Lives in Core, resolves IDs for VFS operations
pub struct NodeRegistry {
    id_to_path: HashMap<NodeId, PathBuf>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            id_to_path: HashMap::new()
        }
    }

    /// Register a path and get its NodeId
    pub fn register(&mut self, path: PathBuf) -> NodeId {
        todo!()
    }

    /// Register multiple paths
    pub fn register_batch(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> Vec<NodeId> {
        todo!()
    }

    /// Resolve NodeId to PathBuf
    pub fn resolve(&self, id: NodeId) -> Option<&PathBuf> {
        todo!()
    }

    /// Resolve multiple NodeIds
    pub fn resolve_batch(&self, ids: &[NodeId]) -> Vec<Option<&PathBuf>> {
        todo!()
    }

    /// Get NodeId for a path (if registered)
    pub fn get_id(&self, path: &PathBuf) -> Option<NodeId> {
        todo!()
    }

    /// Remove a path from registry
    pub fn unregister(&mut self, id: NodeId) -> Option<PathBuf> {
        todo!()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        todo!()
    }

    /// Number of registered paths
    pub fn len(&self) -> usize {
        todo!()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        todo!()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
