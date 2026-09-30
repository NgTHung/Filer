//! Tests for the Location-based registry.

use crate::errors::{ErrorCode, ErrorKind};
use crate::model::location::{
    Location, LocationDescriptor, LocationId, LocationRef, LocationRoute,
};
use crate::model::registry::NodeRegistry;
use std::path::PathBuf;

#[test]
fn registry_starts_empty() {
    let registry = NodeRegistry::new();

    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
}

#[test]
fn registry_default_starts_empty() {
    let registry = NodeRegistry::default();

    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
}

#[test]
fn registry_registers_location_and_resolves_descriptor() {
    let registry = NodeRegistry::new();
    let location = Location::local("/home/user/test.txt");
    let descriptor = location.descriptor().clone();

    let id = registry.register_location(location);

    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    assert_eq!(registry.resolve_location(id), Some(descriptor));
}

#[test]
fn registry_registering_same_location_twice_is_idempotent() {
    let registry = NodeRegistry::new();

    let first = registry.register_location(Location::local("/home/user/test.txt"));
    let second = registry.register_location(Location::local("/home/user/test.txt"));

    assert_eq!(first, second);
    assert_eq!(registry.len(), 1);
}

#[test]
fn registry_tracks_distinct_locations() {
    let registry = NodeRegistry::new();

    let first = registry.register_location(Location::local("/home/user/test.txt"));
    let second = registry.register_location(Location::local("/home/user/other.txt"));

    assert_ne!(first, second);
    assert_eq!(registry.len(), 2);
}

#[test]
fn registry_returns_none_for_unknown_location_id() {
    let registry = NodeRegistry::new();

    assert_eq!(registry.resolve_location(LocationId(999)), None);
}

#[test]
fn registry_resolves_id_only_reference_from_registered_location() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/example.txt");
    let id = location.id();
    let descriptor = location.descriptor().clone();
    registry.register_location(location);

    let resolved = registry
        .resolve_location_ref(&LocationRef::id_only(id))
        .unwrap();

    assert_eq!(resolved.id(), id);
    assert_eq!(resolved.descriptor(), &descriptor);
}

#[test]
fn registry_registers_descriptor_only_reference() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local("/tmp/descriptor-only.txt");

    let resolved = registry
        .resolve_location_ref(&LocationRef::descriptor_only(descriptor.clone()))
        .unwrap();

    assert_eq!(resolved.descriptor(), &descriptor);
    assert_eq!(registry.resolve_location(resolved.id()), Some(descriptor));
}

#[test]
fn registry_recovers_full_reference_with_stale_id() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local("/tmp/recovered.txt");

    let resolved = registry
        .resolve_location_ref(&LocationRef::Full {
            id: LocationId(999),
            descriptor: descriptor.clone(),
        })
        .unwrap();

    assert_ne!(resolved.id(), LocationId(999));
    assert_eq!(resolved.descriptor(), &descriptor);
    assert_eq!(registry.resolve_location(resolved.id()), Some(descriptor));
}

#[test]
fn registry_rejects_unknown_id_only_reference() {
    let registry = NodeRegistry::new();

    let error = registry
        .resolve_location_ref(&LocationRef::id_only(LocationId(999)))
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::InvalidLocation);
    assert_eq!(error.code(), ErrorCode::LocationUnresolved);
}

#[test]
fn registry_preserves_segmented_descriptor_ref() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local("/tmp/bundle.zip")
        .archive_member("vendor.tar")
        .archive_member("src/main.rs");

    let resolved = registry
        .resolve_location_ref(&LocationRef::descriptor_only(descriptor.clone()))
        .unwrap();

    assert_eq!(resolved.descriptor(), &descriptor);
    assert_eq!(registry.resolve_location(resolved.id()), Some(descriptor));
}

#[test]
fn registry_resolves_and_caches_location_route() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/project");
    let id = location.id();
    registry.register_location(location);

    assert_eq!(registry.cached_location_route(id), None);
    assert_eq!(
        registry.resolve_location_route(id).unwrap(),
        LocationRoute::DirectPath {
            path: PathBuf::from("/tmp/project")
        }
    );
    assert_eq!(
        registry.cached_location_route(id),
        Some(LocationRoute::DirectPath {
            path: PathBuf::from("/tmp/project")
        })
    );
}

#[test]
fn registry_local_path_constructor_registers_location() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/tmp/location.txt");

    let location = registry.location_for_path(path.clone());

    assert_eq!(location.descriptor(), &LocationDescriptor::local(path));
    assert_eq!(
        registry.resolve_location(location.id()),
        Some(location.into_descriptor())
    );
    assert_eq!(registry.len(), 1);
}

#[test]
fn registry_clear_removes_descriptors_and_route_cache() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/project");
    let id = location.id();
    registry.register_location(location);
    registry.resolve_location_route(id).unwrap();

    registry.clear();

    assert_eq!(registry.resolve_location(id), None);
    assert_eq!(registry.cached_location_route(id), None);
    assert!(registry.is_empty());
}
