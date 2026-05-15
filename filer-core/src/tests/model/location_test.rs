use std::path::PathBuf;

use crate::errors::{CoreError, ErrorKind};
use crate::model::location::{Location, LocationDescriptor, LocationId, LocationRef, ProviderRef};
use crate::model::registry::NodeRegistry;

#[test]
fn local_descriptor_uses_file_scheme_and_local_provider() {
    let path = PathBuf::from("/tmp/example.txt");
    let descriptor = LocationDescriptor::local(path.clone());

    assert_eq!(descriptor.scheme(), "file");
    assert_eq!(descriptor.provider(), &ProviderRef::Local);
    assert_eq!(descriptor.path(), path.as_path());
    assert_eq!(descriptor.as_local_path(), Some(path.as_path()));
    assert_eq!(descriptor.display_path(), path.display().to_string());
}

#[test]
fn local_location_ids_are_stable_for_same_path() {
    let first = Location::local(PathBuf::from("/tmp/example.txt"));
    let second = Location::local(PathBuf::from("/tmp/example.txt"));
    let different = Location::local(PathBuf::from("/tmp/other.txt"));

    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), different.id());
}

#[test]
fn provider_descriptor_stores_profile_identity_without_credentials() {
    let descriptor = LocationDescriptor::provider_profile("sftp", "work", "/home/me/project");

    assert_eq!(descriptor.scheme(), "sftp");
    assert_eq!(
        descriptor.provider(),
        &ProviderRef::Profile("work".to_string())
    );
    assert_eq!(
        descriptor.path(),
        PathBuf::from("/home/me/project").as_path()
    );
    assert_eq!(descriptor.as_local_path(), None);
    assert!(!descriptor.display_path().contains('@'));
    assert!(!descriptor.display_path().contains("password"));
}

#[test]
fn location_ref_supports_id_descriptor_and_full_modes() {
    let location = Location::local(PathBuf::from("/tmp/example.txt"));
    let id = location.id();
    let descriptor = location.descriptor().clone();

    let id_only = LocationRef::id_only(id);
    assert_eq!(id_only.id(), Some(id));
    assert_eq!(id_only.descriptor(), None);

    let descriptor_only = LocationRef::descriptor_only(descriptor.clone());
    assert_eq!(descriptor_only.id(), None);
    assert_eq!(descriptor_only.descriptor(), Some(&descriptor));

    let full = LocationRef::from_location(&location);
    assert_eq!(full.id(), Some(id));
    assert_eq!(full.descriptor(), Some(&descriptor));
}

#[test]
fn registry_resolves_location_ref_by_id_fast_path() {
    let registry = NodeRegistry::new();
    let location = Location::local(PathBuf::from("/tmp/example.txt"));
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
fn registry_recovers_unknown_id_from_descriptor() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local(PathBuf::from("/tmp/recovered.txt"));
    let location_ref = LocationRef::new(Some(LocationId(999)), Some(descriptor.clone()));

    let resolved = registry.resolve_location_ref(&location_ref).unwrap();

    assert_ne!(resolved.id(), LocationId(999));
    assert_eq!(resolved.descriptor(), &descriptor);
    assert_eq!(
        registry.resolve_location(resolved.id()).unwrap(),
        descriptor
    );
}

#[test]
fn registry_rejects_unknown_id_without_descriptor() {
    let registry = NodeRegistry::new();
    let err = registry
        .resolve_location_ref(&LocationRef::id_only(LocationId(999)))
        .unwrap_err();

    assert!(matches!(err, CoreError::InvalidLocation(_)));
    assert_eq!(err.kind(), ErrorKind::InvalidLocation);
}

#[test]
fn non_local_provider_descriptor_has_no_local_path() {
    let descriptor = LocationDescriptor::provider_profile("s3", "assets", "bucket/prefix");

    assert_eq!(descriptor.as_local_path(), None);
}
