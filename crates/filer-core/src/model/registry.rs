use std::path::PathBuf;
use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::errors::CoreError;

use super::location::{Location, LocationDescriptor, LocationId, LocationRef, LocationRoute};

/// Internal registry for reconstructable Location descriptors and routes.
///
/// Public provider-aware transport should prefer `LocationRef::Full` or
/// `LocationRef::Descriptor` over id-only lookup.
#[derive(Clone, Debug)]
pub struct NodeRegistry {
    id_to_location: Arc<scc::HashMap<LocationId, LocationDescriptor, RandomState>>,
    id_to_location_route: Arc<scc::HashMap<LocationId, LocationRoute, RandomState>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            id_to_location: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            id_to_location_route: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.id_to_location.clear_sync();
        self.id_to_location_route.clear_sync();
    }

    /// Number of registered Location descriptors
    pub fn len(&self) -> usize {
        self.id_to_location.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn register_location(&self, location: Location) -> LocationId {
        let id = location.id();
        let _ = self
            .id_to_location
            .insert_sync(id, location.into_descriptor());
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
