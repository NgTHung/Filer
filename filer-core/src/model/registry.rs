use std::path::PathBuf;
use std::sync::Arc;

use crate::FileNode;

use super::node::NodeId;

/// Registry that maps NodeId to PathBuf
/// Lives in Core, resolves IDs for VFS operations
#[derive(Clone, Debug)]
pub struct NodeRegistry {
    id_to_path: Arc<scc::HashMap<NodeId, PathBuf>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            id_to_path: Arc::new(scc::HashMap::new()),
        }
    }

    /// Register a path and get its NodeId
    pub fn register(self, path: PathBuf) -> NodeId {
        let hash = NodeId::from_path(&path);
        let _ = self.id_to_path.insert_sync(hash, path);
        hash
    }
    
    /// Register multiple paths
    pub fn register_batch(self, paths: &Vec<PathBuf>) -> Vec<NodeId> {
        paths.into_iter().map(|v| {
            let hash = NodeId::from_path(&v);
            let _ = self.id_to_path.insert_sync(hash, v.clone());
            hash
        }).collect()
    }

    pub fn register_batch_file_node(self, paths: &Vec<FileNode>) -> Vec<NodeId> {
        paths.into_iter().map(|v| {
            let hash = NodeId::from_path(&v.path);
            let _ = self.id_to_path.insert_sync(hash, v.path.clone());
            hash
        }).collect()
    }


    /// Resolve NodeId to PathBuf
    pub fn resolve(&self, id: NodeId) -> Option<PathBuf> {
        self.id_to_path.read_sync(&id, |_, v| v.clone())
    }

    /// Resolve multiple NodeIds
    pub fn resolve_batch(&self, ids: &[NodeId]) -> Vec<Option<PathBuf>> {
        ids.iter().map(|v| self.resolve(*v)).collect()
    }

    /// Get NodeId for a path (if registered)
    pub fn get_id(&self, path: &PathBuf) -> Option<NodeId> {
        let key = NodeId::from_path(path);
        if self.id_to_path.contains_sync(&key) {
            Some(key)
        } else {
            None
        }
    }

    /// Remove a path from registry
    pub fn unregister(&self, id: NodeId) -> Option<PathBuf> {
        self.id_to_path.remove_sync(&id).map(|(_, v)| v)
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.id_to_path.clear_sync();
    }

    /// Number of registered paths
    pub fn len(&self) -> usize {
        self.id_to_path.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
