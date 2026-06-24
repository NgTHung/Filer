use std::path::{Path, PathBuf};
use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::FileNode;
use crate::errors::CoreError;

use super::location::{Location, LocationDescriptor, LocationId, LocationRef, LocationRoute};
use super::node::NodeId;

/// Internal registry that bridges compatibility handles to filesystem identity.
///
/// `NodeId` entries are direct-local cache and compatibility handles.
/// `LocationId` entries cache reconstructable `LocationDescriptor` data and
/// derived routes. Public provider-aware transport should prefer
/// `LocationRef::Full` or `LocationRef::Descriptor` over id-only lookup.
#[derive(Clone, Debug)]
pub struct NodeRegistry {
    id_to_path: Arc<scc::HashMap<NodeId, PathBuf, RandomState>>,
    id_to_node_location: Arc<scc::HashMap<NodeId, LocationDescriptor, RandomState>>,
    id_to_location: Arc<scc::HashMap<LocationId, LocationDescriptor, RandomState>>,
    id_to_location_route: Arc<scc::HashMap<LocationId, LocationRoute, RandomState>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            id_to_path: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            id_to_node_location: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            id_to_location: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            id_to_location_route: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    /// Register a path and get its NodeId
    pub fn register(self, path: PathBuf) -> NodeId {
        let hash = NodeId::from_path(&path);
        let location = Location::local(path.clone());
        let _ = self.id_to_path.insert_sync(hash, path);
        self.register_location(location.clone());
        let _ = self
            .id_to_node_location
            .insert_sync(hash, location.into_descriptor());
        hash
    }

    /// Register multiple paths
    pub fn register_batch(self, paths: &[PathBuf]) -> Vec<NodeId> {
        paths
            .iter()
            .map(|v| {
                let hash = NodeId::from_path(v);
                let location = Location::local(v.clone());
                let _ = self.id_to_path.insert_sync(hash, v.clone());
                self.register_location(location.clone());
                let _ = self
                    .id_to_node_location
                    .insert_sync(hash, location.into_descriptor());
                hash
            })
            .collect()
    }

    pub fn register_batch_file_node(self, paths: &[FileNode]) -> Vec<NodeId> {
        paths
            .iter()
            .map(|v| {
                let hash = NodeId::from_path(&v.path);
                let location = Location::local(v.path.clone());
                let _ = self.id_to_path.insert_sync(hash, v.path.clone());
                self.register_location(location.clone());
                let _ = self
                    .id_to_node_location
                    .insert_sync(hash, location.into_descriptor());
                hash
            })
            .collect()
    }

    /// Resolve NodeId to PathBuf
    pub fn resolve(&self, id: NodeId) -> Option<PathBuf> {
        self.id_to_path.read_sync(&id, |_, v| v.clone())
    }

    pub fn resolve_node_location(&self, id: NodeId) -> Option<LocationRef> {
        self.id_to_node_location
            .read_sync(&id, |_, descriptor| {
                let location = Location::new(descriptor.clone());
                LocationRef::from_location(&location)
            })
            .or_else(|| {
                self.resolve(id).map(|path| {
                    let location = self.location_for_path(path);
                    LocationRef::from_location(&location)
                })
            })
    }

    /// Resolve multiple NodeIds
    pub fn resolve_batch(&self, ids: &[NodeId]) -> Vec<Option<PathBuf>> {
        ids.iter().map(|v| self.resolve(*v)).collect()
    }

    /// Get NodeId for a path (if registered)
    pub fn get_id(&self, path: &Path) -> Option<NodeId> {
        let key = NodeId::from_path(path);
        if self.id_to_path.contains_sync(&key) {
            Some(key)
        } else {
            None
        }
    }

    /// Remove a path from registry
    pub fn unregister(&self, id: NodeId) -> Option<PathBuf> {
        let _ = self.id_to_node_location.remove_sync(&id);
        self.id_to_path.remove_sync(&id).map(|(_, v)| v)
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.id_to_path.clear_sync();
        self.id_to_node_location.clear_sync();
        self.id_to_location.clear_sync();
        self.id_to_location_route.clear_sync();
    }

    /// Number of registered paths
    pub fn len(&self) -> usize {
        self.id_to_path.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn have_par(&self, id: NodeId) -> Option<bool> {
        self.resolve(id).map(|path| path.parent().is_some())
    }

    pub fn get_par(&self, id: NodeId) -> Option<PathBuf> {
        if let Some(path) = self.resolve(id) {
            path.clone().parent().map(|p| p.to_path_buf())
        } else {
            None
        }
    }

    pub fn register_location(&self, location: Location) -> LocationId {
        let id = location.id();
        let _ = self
            .id_to_location
            .insert_sync(id, location.into_descriptor());
        id
    }

    pub fn register_location_node(&self, location: Location) -> Result<NodeId, CoreError> {
        let route = location.route();
        let path = route.require_direct_path()?.to_path_buf();
        let id = NodeId::from_path(&path);
        let descriptor = location.descriptor().clone();
        self.register_location(location);
        let _ = self.id_to_path.insert_sync(id, path);
        let _ = self.id_to_node_location.insert_sync(id, descriptor);
        Ok(id)
    }

    pub fn register_segmented_location_node(&self, location: Location) -> NodeId {
        let id = NodeId::from_path(Path::new(&location.descriptor().display_path()));
        let descriptor = location.descriptor().clone();
        self.register_location(location);
        let _ = self.id_to_node_location.insert_sync(id, descriptor);
        id
    }

    pub fn resolve_location(&self, id: LocationId) -> Option<LocationDescriptor> {
        self.id_to_location.read_sync(&id, |_, v| v.clone())
    }

    pub fn cached_location_route(&self, id: LocationId) -> Option<LocationRoute> {
        self.id_to_location_route.read_sync(&id, |_, v| v.clone())
    }

    pub fn resolve_location_route(&self, id: LocationId) -> Result<LocationRoute, CoreError> {
        if let Some(route) = self.cached_location_route(id) {
            return Ok(route);
        }

        let descriptor = self
            .resolve_location(id)
            .ok_or_else(|| CoreError::location_unresolved(id))?;
        let route = descriptor.route();
        let _ = self.id_to_location_route.insert_sync(id, route.clone());
        Ok(route)
    }

    pub fn resolve_location_ref(&self, location_ref: &LocationRef) -> Result<Location, CoreError> {
        match location_ref {
            LocationRef::Id(id) => self
                .resolve_location(*id)
                .map(Location::new)
                .ok_or_else(|| CoreError::location_unresolved(*id)),
            LocationRef::Descriptor(descriptor) => {
                let location = Location::new(descriptor.clone());
                self.register_location(location.clone());
                Ok(location)
            }
            LocationRef::Full { id, descriptor } => {
                if let Some(registered) = self.resolve_location(*id) {
                    Ok(Location::new(registered))
                } else {
                    let location = Location::new(descriptor.clone());
                    self.register_location(location.clone());
                    Ok(location)
                }
            }
        }
    }

    pub fn location_for_path(&self, path: PathBuf) -> Location {
        let location = Location::local(path);
        self.register_location(location.clone());
        location
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
