use std::path::PathBuf;

use crate::errors::ErrorCode;
use crate::model::capability::{
    LocationCapabilityError, LocationConflictPolicy, LocationCrossProviderPolicy,
    LocationWatchReliability, operation_capability_for_location, watch_capability_for_location,
};
use crate::model::location::{Location, LocationDescriptor, LocationId, LocationRef};
use crate::model::operation::OperationKind;
use crate::model::registry::NodeRegistry;
use crate::vfs::provider::Capabilities;

fn capabilities(write: bool, watch: bool) -> Capabilities {
    Capabilities {
        read: true,
        write,
        watch,
        search: false,
    }
}

#[test]
fn direct_local_watch_supported_when_provider_can_watch() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/watchable");

    let capability = watch_capability_for_location(
        &LocationRef::from_location(&location),
        &registry,
        capabilities(false, true),
    )
    .unwrap();

    assert!(capability.supported);
    assert!(capability.recursive);
    assert!(capability.location_events);
    assert_eq!(capability.reliability, LocationWatchReliability::BestEffort);
    assert_eq!(capability.unsupported, None);
}

#[test]
fn direct_local_watch_reports_unsupported_when_provider_cannot_watch() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/not-watchable");

    let capability = watch_capability_for_location(
        &LocationRef::from_location(&location),
        &registry,
        capabilities(false, false),
    )
    .unwrap();

    assert!(!capability.supported);
    assert!(!capability.recursive);
    assert!(!capability.location_events);
    assert_eq!(
        capability.unsupported,
        Some(LocationCapabilityError::WatchUnsupported)
    );
}

#[test]
fn direct_local_operation_supported_when_provider_can_write() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/writable");

    let capability = operation_capability_for_location(
        &LocationRef::from_location(&location),
        OperationKind::CreateFile,
        &registry,
        capabilities(true, false),
    )
    .unwrap();

    assert!(capability.supported);
    assert_eq!(capability.operation, OperationKind::CreateFile);
    assert_eq!(capability.conflict, LocationConflictPolicy::FailIfExists);
    assert_eq!(
        capability.cross_provider,
        LocationCrossProviderPolicy::Unsupported
    );
    assert!(capability.reports_affected_locations);
    assert_eq!(capability.unsupported, None);
}

#[test]
fn direct_local_operation_reports_unsupported_when_provider_cannot_write() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/read-only");

    let capability = operation_capability_for_location(
        &LocationRef::from_location(&location),
        OperationKind::Delete,
        &registry,
        capabilities(false, false),
    )
    .unwrap();

    assert!(!capability.supported);
    assert!(!capability.reports_affected_locations);
    assert_eq!(
        capability.unsupported,
        Some(LocationCapabilityError::WriteUnsupported)
    );
}

#[test]
fn segmented_location_returns_segmented_unsupported_for_watch() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local("/tmp/archive.zip").archive_member("inside");

    let error = watch_capability_for_location(
        &LocationRef::descriptor_only(descriptor),
        &registry,
        capabilities(true, true),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::LocationSegmentedUnsupported);
}

#[test]
fn segmented_location_returns_segmented_unsupported_for_operation() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local("/tmp/archive.zip").archive_member("inside");

    let error = operation_capability_for_location(
        &LocationRef::descriptor_only(descriptor),
        OperationKind::Rename,
        &registry,
        capabilities(true, true),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::LocationSegmentedUnsupported);
}

#[test]
fn provider_profile_returns_unsupported_provider_for_watch() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::provider_profile("sftp", "work", "/home/me");

    let error = watch_capability_for_location(
        &LocationRef::descriptor_only(descriptor),
        &registry,
        capabilities(true, true),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::UnsupportedProvider);
}

#[test]
fn provider_profile_returns_unsupported_provider_for_operation() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::ephemeral("memory", "session", "/virtual");

    let error = operation_capability_for_location(
        &LocationRef::descriptor_only(descriptor),
        OperationKind::Move,
        &registry,
        capabilities(true, true),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::UnsupportedProvider);
}

#[test]
fn id_only_missing_location_returns_location_unresolved() {
    let registry = NodeRegistry::new();
    let missing_id = LocationId(42);

    let error = watch_capability_for_location(
        &LocationRef::id_only(missing_id),
        &registry,
        capabilities(true, true),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::LocationUnresolved);
}

#[test]
fn descriptor_capability_check_does_not_register_location() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local(PathBuf::from("/tmp/no-side-effect"));
    let id = Location::new(descriptor.clone()).id();

    let capability = watch_capability_for_location(
        &LocationRef::descriptor_only(descriptor),
        &registry,
        capabilities(false, true),
    )
    .unwrap();

    assert!(capability.supported);
    assert!(
        registry.resolve_location(id).is_none(),
        "capability checks must not register descriptor-only locations"
    );
}
