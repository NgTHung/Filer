//! # Operation Targets
//!
//! This module resolves operation locations into direct-local provider paths.
//! Write operations stay conservative until provider routing can express
//! segmented or non-local destinations without losing capability context.
//!
//! ```
//! use filer_core::modules::operations::target::resolve_direct_target;
//! use filer_core::model::registry::NodeRegistry;
//! use filer_core::{Capabilities, Location, LocationRef, OperationKind};
//!
//! let registry = NodeRegistry::new();
//! let location = LocationRef::from_location(&Location::local("/tmp/example.txt"));
//! let path = resolve_direct_target(
//!     &registry,
//!     &location,
//!     OperationKind::Delete,
//!     Capabilities {
//!         read: true,
//!         write: true,
//!         watch: false,
//!         search: false,
//!     },
//! )?;
//! assert_eq!(path, std::path::PathBuf::from("/tmp/example.txt"));
//! # Ok::<(), filer_core::CoreError>(())
//! ```

use std::path::PathBuf;

use crate::CoreError;
use crate::model::capability::{LocationCapabilityError, operation_capability_for_location};
use crate::model::location::{Location, LocationRef};
use crate::model::operation::OperationKind;
use crate::model::registry::NodeRegistry;
use crate::vfs::provider::Capabilities;

pub fn resolve_direct_target(
    registry: &NodeRegistry,
    location: &LocationRef,
    kind: OperationKind,
    capabilities: Capabilities,
) -> Result<PathBuf, CoreError> {
    let capability =
        operation_capability_for_location(location, kind.clone(), registry, capabilities)?;
    if !capability.supported {
        let provider = capability
            .location
            .descriptor()
            .map(|descriptor| descriptor.provider().clone())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "Resolved capability location is missing its provider descriptor",
                )
            })?;
        let missing = capability
            .unsupported
            .clone()
            .unwrap_or(LocationCapabilityError::OperationUnsupported(kind));
        return Err(CoreError::provider_capability(
            provider,
            capability.location,
            missing,
        ));
    }

    let location = registry.resolve_location_ref(location)?;
    Ok(location.route().require_direct_path()?.to_path_buf())
}

pub fn resolve_direct_targets(
    registry: &NodeRegistry,
    locations: &[LocationRef],
    kind: OperationKind,
    capabilities: Capabilities,
) -> Result<Vec<PathBuf>, CoreError> {
    locations
        .iter()
        .map(|location| resolve_direct_target(registry, location, kind.clone(), capabilities))
        .collect()
}

pub fn affected_location(registry: &NodeRegistry, path: PathBuf) -> LocationRef {
    LocationRef::from_location(&registry.location_for_path(path))
}

pub fn path_location(path: PathBuf) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}
