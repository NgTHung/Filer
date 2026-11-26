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

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

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
    if locations.is_empty() {
        return Err(CoreError::invalid_input(
            "Expected at least one operation target",
        ));
    }
    locations
        .iter()
        .map(|location| resolve_direct_target(registry, location, kind.clone(), capabilities))
        .collect()
}

pub(super) fn resolve_named_target(
    registry: &NodeRegistry,
    location: &LocationRef,
    name: &str,
    kind: OperationKind,
    capabilities: Capabilities,
) -> Result<PathBuf, CoreError> {
    // Joining a rooted name replaces its parent, and components can normalize away separators.
    if Path::new(name).file_name() != Some(OsStr::new(name)) || name.as_bytes().contains(&0) {
        return Err(CoreError::invalid_input(
            "Expected one native filename component",
        ));
    }
    #[cfg(windows)]
    if invalid_windows_name(name) {
        return Err(CoreError::invalid_input("Invalid Windows filename"));
    }

    resolve_direct_target(registry, location, kind, capabilities)
}

#[cfg(windows)]
fn invalid_windows_name(name: &str) -> bool {
    if name.ends_with([' ', '.'])
        || name.contains(|c| {
            matches!(
                c,
                '\u{1}'..='\u{1f}' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
            )
        })
    {
        return true;
    }

    // Win32 can interpret a DOS device name with an extension as the device itself.
    let stem = name
        .split_once('.')
        .map_or(name, |(stem, _)| stem)
        .trim_end_matches(' ');
    ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
        .iter()
        .any(|device| stem.eq_ignore_ascii_case(device))
        || stem.split_at_checked(3).is_some_and(|(prefix, suffix)| {
            (prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT"))
                && matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
        })
}

pub fn affected_location(registry: &NodeRegistry, path: PathBuf) -> LocationRef {
    LocationRef::from_location(&registry.location_for_path(path))
}

pub fn path_location(path: PathBuf) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}
