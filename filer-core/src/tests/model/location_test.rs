use std::path::PathBuf;

use crate::errors::{ErrorCode, ErrorKind};
use crate::model::location::{
    Location, LocationDescriptor, LocationId, LocationRef, LocationRoute, LocationSegment,
    ProviderRef,
};
use crate::model::node::{FileNode, NodeEntry, NodeId, NodeKind, NodeMeta};
use crate::model::registry::NodeRegistry;

#[test]
fn local_descriptor_uses_file_scheme_and_local_provider() {
    let path = PathBuf::from("/tmp/example.txt");
    let descriptor = LocationDescriptor::local(path.clone());

    assert_eq!(descriptor.scheme(), "file");
    assert_eq!(descriptor.provider(), &ProviderRef::Local);
    assert_eq!(descriptor.root(), path.as_path());
    assert!(descriptor.segments().is_empty());
    assert!(!descriptor.is_segmented());
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
        descriptor.root(),
        PathBuf::from("/home/me/project").as_path()
    );
    assert!(descriptor.segments().is_empty());
    assert_eq!(descriptor.as_local_path(), None);
    assert!(!descriptor.display_path().contains('@'));
    assert!(!descriptor.display_path().contains("password"));
}

#[test]
fn location_ref_constructors_return_expected_variants() {
    let location = Location::local(PathBuf::from("/tmp/example.txt"));
    let id = location.id();
    let descriptor = location.descriptor().clone();

    let id_only = LocationRef::id_only(id);
    assert_eq!(id_only, LocationRef::Id(id));
    assert_eq!(id_only.id(), Some(id));
    assert_eq!(id_only.descriptor(), None);

    let descriptor_only = LocationRef::descriptor_only(descriptor.clone());
    assert_eq!(descriptor_only, LocationRef::Descriptor(descriptor.clone()));
    assert_eq!(descriptor_only.id(), None);
    assert_eq!(descriptor_only.descriptor(), Some(&descriptor));

    let full = LocationRef::from_location(&location);
    assert_eq!(
        full,
        LocationRef::Full {
            id,
            descriptor: descriptor.clone()
        }
    );
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
    let location_ref = LocationRef::Full {
        id: LocationId(999),
        descriptor: descriptor.clone(),
    };

    let resolved = registry.resolve_location_ref(&location_ref).unwrap();

    assert_ne!(resolved.id(), LocationId(999));
    assert_eq!(resolved.descriptor(), &descriptor);
    assert_eq!(
        registry.resolve_location(resolved.id()).unwrap(),
        descriptor
    );
}

#[test]
fn registry_registers_descriptor_ref() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local(PathBuf::from("/tmp/descriptor-only.txt"));

    let resolved = registry
        .resolve_location_ref(&LocationRef::descriptor_only(descriptor.clone()))
        .unwrap();

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

    assert_eq!(err.kind(), ErrorKind::InvalidLocation);
    assert_eq!(err.code(), ErrorCode::LocationUnresolved);
}

#[test]
fn non_local_provider_descriptor_has_no_local_path() {
    let descriptor = LocationDescriptor::provider_profile("s3", "assets", "bucket/prefix");

    assert_eq!(descriptor.as_local_path(), None);
}

#[test]
fn unsegmented_local_descriptor_routes_to_direct_path() {
    let descriptor = LocationDescriptor::local("/tmp/project");
    let route = descriptor.route();

    assert_eq!(
        route,
        LocationRoute::DirectPath {
            path: PathBuf::from("/tmp/project")
        }
    );
    assert_eq!(
        route.as_direct_path(),
        Some(PathBuf::from("/tmp/project").as_path())
    );
    assert!(!route.is_segmented());
    assert_eq!(
        route.require_direct_path().unwrap(),
        PathBuf::from("/tmp/project").as_path()
    );
}

#[test]
fn location_routes_through_its_descriptor() {
    let location = Location::local("/tmp/project");

    assert_eq!(
        location.route(),
        LocationRoute::DirectPath {
            path: PathBuf::from("/tmp/project")
        }
    );
}

#[test]
fn archive_member_segments_are_ordered_layers_after_root() {
    let descriptor = LocationDescriptor::provider_profile("sftp", "work", "/home/me/bundle.zip")
        .archive_member("vendor.tar")
        .archive_member("src/main.rs");

    assert_eq!(descriptor.root(), PathBuf::from("/home/me/bundle.zip"));
    assert_eq!(
        descriptor.segments(),
        &[
            LocationSegment::ArchiveMember {
                path: PathBuf::from("vendor.tar")
            },
            LocationSegment::ArchiveMember {
                path: PathBuf::from("src/main.rs")
            }
        ]
    );
    assert!(descriptor.is_segmented());
    assert_eq!(
        descriptor.display_path(),
        "/home/me/bundle.zip!/vendor.tar!/src/main.rs"
    );
}

#[test]
fn virtual_segments_are_preserved_as_future_vfs_layers() {
    let descriptor =
        LocationDescriptor::local("/workspace").with_segment(LocationSegment::Virtual {
            scheme: "git".to_string(),
            path: PathBuf::from("HEAD:src/lib.rs"),
        });

    assert_eq!(
        descriptor.segments(),
        &[LocationSegment::Virtual {
            scheme: "git".to_string(),
            path: PathBuf::from("HEAD:src/lib.rs")
        }]
    );
    assert_eq!(descriptor.display_path(), "/workspace!/git:HEAD:src/lib.rs");
}

#[test]
fn location_id_includes_ordered_segments() {
    let root = LocationDescriptor::local("/tmp/bundle.zip");
    let first = Location::new(root.clone().archive_member("vendor.tar"));
    let second = Location::new(root.clone().archive_member("src.tar"));
    let nested_a = Location::new(
        root.clone()
            .archive_member("vendor.tar")
            .archive_member("src/main.rs"),
    );
    let nested_b = Location::new(
        root.archive_member("src/main.rs")
            .archive_member("vendor.tar"),
    );

    assert_ne!(first.id(), second.id());
    assert_ne!(nested_a.id(), nested_b.id());
}

#[test]
fn segmented_local_descriptor_has_no_direct_local_path() {
    let descriptor = LocationDescriptor::local("/tmp/bundle.zip").archive_member("inside.txt");

    assert_eq!(descriptor.as_local_path(), None);
}

#[test]
fn segmented_local_descriptor_routes_to_segmented_target() {
    let descriptor = LocationDescriptor::local("/tmp/bundle.zip")
        .archive_member("vendor.tar")
        .archive_member("src/main.rs");
    let route = descriptor.route();

    assert_eq!(
        route,
        LocationRoute::Segmented {
            root: PathBuf::from("/tmp/bundle.zip"),
            segments: vec![
                LocationSegment::ArchiveMember {
                    path: PathBuf::from("vendor.tar")
                },
                LocationSegment::ArchiveMember {
                    path: PathBuf::from("src/main.rs")
                }
            ]
        }
    );
    assert_eq!(route.as_direct_path(), None);
    assert!(route.is_segmented());
}

#[test]
fn require_direct_path_rejects_segmented_locations() {
    let route = LocationDescriptor::local("/tmp/bundle.zip")
        .archive_member("inside.txt")
        .route();
    let err = route.require_direct_path().unwrap_err();

    assert_eq!(err.kind(), ErrorKind::InvalidLocation);
    assert_eq!(err.code(), ErrorCode::LocationSegmentedUnsupported);
}

#[test]
fn profile_provider_routes_to_unsupported_provider() {
    let descriptor = LocationDescriptor::provider_profile("sftp", "work", "/home/me/project");

    assert_eq!(
        descriptor.route(),
        LocationRoute::UnsupportedProvider {
            scheme: "sftp".to_string(),
            provider: ProviderRef::Profile("work".to_string()),
            root: PathBuf::from("/home/me/project")
        }
    );
}

#[test]
fn ephemeral_provider_routes_to_unsupported_provider() {
    let descriptor = LocationDescriptor::ephemeral("archive-cache", "session-archive", "/member");

    assert_eq!(
        descriptor.route(),
        LocationRoute::UnsupportedProvider {
            scheme: "archive-cache".to_string(),
            provider: ProviderRef::Ephemeral("session-archive".to_string()),
            root: PathBuf::from("/member")
        }
    );
}

#[test]
fn unsupported_provider_route_ignores_display_path() {
    let plain = LocationDescriptor::provider_profile("s3", "assets", "bucket/prefix");
    let display = LocationDescriptor::provider_profile("s3", "assets", "bucket/prefix")
        .with_display_path("Assets");

    assert_eq!(plain.route(), display.route());
}

#[test]
fn display_path_does_not_affect_location_id() {
    let plain = Location::new(LocationDescriptor::local("/tmp/display.txt"));
    let display = Location::new(
        LocationDescriptor::local("/tmp/display.txt").with_display_path("Pretty Name"),
    );

    assert_eq!(plain.id(), display.id());
}

#[test]
fn provider_identity_affects_location_id() {
    let local = Location::new(LocationDescriptor::local("/shared/path"));
    let first_profile = Location::new(LocationDescriptor::provider_profile(
        "sftp",
        "work-a",
        "/shared/path",
    ));
    let second_profile = Location::new(LocationDescriptor::provider_profile(
        "sftp",
        "work-b",
        "/shared/path",
    ));

    assert_ne!(local.id(), first_profile.id());
    assert_ne!(first_profile.id(), second_profile.id());
}

#[test]
fn local_descriptor_does_not_canonicalize_path() {
    let path = PathBuf::from("/tmp/does-not-need-to-exist/../file.txt");
    let descriptor = LocationDescriptor::local(path.clone());

    assert_eq!(descriptor.root(), path.as_path());
}

#[test]
fn ephemeral_provider_identity_is_session_local() {
    let descriptor = LocationDescriptor::ephemeral("archive-cache", "session-archive", "/member");

    assert_eq!(descriptor.scheme(), "archive-cache");
    assert_eq!(
        descriptor.provider(),
        &ProviderRef::Ephemeral("session-archive".to_string())
    );
    assert_eq!(descriptor.as_local_path(), None);
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
    assert_eq!(
        registry.resolve_location(resolved.id()).unwrap(),
        descriptor
    );
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
        registry.cached_location_route(id).unwrap(),
        LocationRoute::DirectPath {
            path: PathBuf::from("/tmp/project")
        }
    );
}

#[test]
fn registry_clears_cached_location_routes() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/project");
    let id = location.id();

    registry.register_location(location);
    registry.resolve_location_route(id).unwrap();
    registry.clear();

    assert_eq!(registry.resolve_location(id), None);
    assert_eq!(registry.cached_location_route(id), None);
}

#[test]
fn registry_resolves_node_location_for_registered_path() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/tmp/location-node.txt");
    let node = registry.clone().register(path.clone());
    let descriptor = LocationDescriptor::local(path);

    let location_ref = registry.resolve_node_location(node).unwrap();

    assert_eq!(
        location_ref.id(),
        Some(LocationId::from_descriptor(&descriptor))
    );
    assert_eq!(location_ref.descriptor(), Some(&descriptor));
}

#[test]
fn registry_registers_direct_location_as_node() {
    let registry = NodeRegistry::new();
    let location = Location::local("/tmp/location-root");
    let id = registry.register_location_node(location.clone()).unwrap();

    assert_eq!(
        registry.resolve(id).unwrap(),
        PathBuf::from("/tmp/location-root")
    );
    assert_eq!(
        registry.resolve_node_location(id).unwrap().id(),
        Some(location.id())
    );
}

#[test]
fn node_entry_carries_location_without_exposing_path() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/tmp/entry.txt");
    let node = FileNode {
        id: NodeId::from_path(&path),
        name: "entry.txt".to_string(),
        path: path.clone(),
        kind: NodeKind::File {
            extension: Some("txt".to_string()),
        },
        size: 42,
        modified: None,
        created: None,
        accessed: None,
        meta: NodeMeta::default(),
    };

    let entry = NodeEntry::from_file_node(node, &registry);

    assert_eq!(entry.name, "entry.txt");
    assert_eq!(entry.extension(), Some("txt"));
    assert_eq!(
        entry.location.descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn registry_recovers_unknown_full_ref_with_segments() {
    let registry = NodeRegistry::new();
    let descriptor = LocationDescriptor::local("/tmp/bundle.zip").archive_member("inside.txt");

    let resolved = registry
        .resolve_location_ref(&LocationRef::Full {
            id: LocationId(999),
            descriptor: descriptor.clone(),
        })
        .unwrap();

    assert_ne!(resolved.id(), LocationId(999));
    assert_eq!(resolved.descriptor(), &descriptor);
    assert_eq!(
        registry.resolve_location(resolved.id()).unwrap(),
        descriptor
    );
}

#[test]
fn segmented_descriptor_round_trips_through_serde() {
    let descriptor = LocationDescriptor::provider_profile("s3", "assets", "bucket/archive.zip")
        .archive_member("images.tar")
        .with_segment(LocationSegment::Virtual {
            scheme: "thumbnail".to_string(),
            path: PathBuf::from("large/photo.png"),
        })
        .with_display_path("asset preview");

    let json = serde_json::to_string(&descriptor).unwrap();
    let decoded: LocationDescriptor = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, descriptor);
    assert_eq!(
        LocationId::from_descriptor(&decoded),
        LocationId::from_descriptor(&descriptor)
    );
}
