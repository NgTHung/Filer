use serde::{Deserialize, Serialize};

use crate::errors::CoreError;
use crate::model::location::{Location, LocationRef};
use crate::model::operation::OperationKind;
use crate::model::registry::NodeRegistry;
use crate::vfs::provider::Capabilities;

/// Why a Location capability is unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocationCapabilityError {
    WatchUnsupported,
    WriteUnsupported,
    OperationUnsupported(OperationKind),
}

/// How reliable a provider watch stream is expected to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocationWatchReliability {
    BestEffort,
}

/// Watch contract for a resolved Location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationWatchCapability {
    pub location: LocationRef,
    pub supported: bool,
    pub recursive: bool,
    pub location_events: bool,
    pub reliability: LocationWatchReliability,
    pub unsupported: Option<LocationCapabilityError>,
}

/// Conflict behavior exposed by the current coarse operation capability model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocationConflictPolicy {
    FailIfExists,
}

/// Cross-provider behavior exposed by the current coarse operation capability model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocationCrossProviderPolicy {
    Unsupported,
}

/// Write/operation contract for a resolved Location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationOperationCapability {
    pub location: LocationRef,
    pub operation: OperationKind,
    pub supported: bool,
    pub conflict: LocationConflictPolicy,
    pub cross_provider: LocationCrossProviderPolicy,
    pub reports_affected_locations: bool,
    pub unsupported: Option<LocationCapabilityError>,
}

impl LocationWatchCapability {
    fn direct_local(location: LocationRef, capabilities: Capabilities) -> Self {
        Self {
            location,
            supported: capabilities.watch,
            recursive: capabilities.watch,
            location_events: capabilities.watch,
            reliability: LocationWatchReliability::BestEffort,
            unsupported: (!capabilities.watch).then_some(LocationCapabilityError::WatchUnsupported),
        }
    }
}

impl LocationOperationCapability {
    fn direct_local(
        location: LocationRef,
        operation: OperationKind,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            location,
            operation,
            supported: capabilities.write,
            conflict: LocationConflictPolicy::FailIfExists,
            cross_provider: LocationCrossProviderPolicy::Unsupported,
            reports_affected_locations: capabilities.write,
            unsupported: (!capabilities.write).then_some(LocationCapabilityError::WriteUnsupported),
        }
    }
}

/// Report whether the current provider can watch a Location.
///
/// This helper is intentionally side-effect free: descriptor/full references
/// are reconstructed without registering new locations. Id-only references
/// require an existing registry entry.
pub fn watch_capability_for_location(
    location_ref: &LocationRef,
    registry: &NodeRegistry,
    capabilities: Capabilities,
) -> Result<LocationWatchCapability, CoreError> {
    let location = resolve_capability_location(location_ref, registry)?;
    location.route().require_direct_path()?;
    Ok(LocationWatchCapability::direct_local(
        LocationRef::from_location(&location),
        capabilities,
    ))
}

/// Report whether the current provider can perform a write operation at a Location.
///
/// This helper is intentionally side-effect free and does not mutate files.
pub fn operation_capability_for_location(
    location_ref: &LocationRef,
    operation: OperationKind,
    registry: &NodeRegistry,
    capabilities: Capabilities,
) -> Result<LocationOperationCapability, CoreError> {
    let location = resolve_capability_location(location_ref, registry)?;
    location.route().require_direct_path()?;
    Ok(LocationOperationCapability::direct_local(
        LocationRef::from_location(&location),
        operation,
        capabilities,
    ))
}

fn resolve_capability_location(
    location_ref: &LocationRef,
    registry: &NodeRegistry,
) -> Result<Location, CoreError> {
    match location_ref {
        LocationRef::Id(id) => registry
            .resolve_location(*id)
            .map(Location::new)
            .ok_or_else(|| CoreError::location_unresolved(*id)),
        LocationRef::Descriptor(descriptor) => Ok(Location::new(descriptor.clone())),
        LocationRef::Full { id, descriptor } => Ok(registry
            .resolve_location(*id)
            .map(Location::new)
            .unwrap_or_else(|| Location::new(descriptor.clone()))),
    }
}
