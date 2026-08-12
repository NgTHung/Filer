//! Tests for model layer

use crate::model::location::{Location, LocationDescriptor};
use crate::model::node::{FileNode, NodeEntry};
use crate::model::registry::NodeRegistry;
use std::path::PathBuf;

#[cfg(unix)]
fn non_utf8_path() -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    PathBuf::from(OsString::from_vec(b"bad-\xFF-name.txt".to_vec()))
}

#[cfg(windows)]
fn non_utf8_path() -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    PathBuf::from(OsString::from_wide(&[
        0x0062, 0x0061, 0x0064, 0x002D, 0xD800, 0x002E, 0x0074, 0x0078, 0x0074,
    ]))
}

#[test]
fn test_location_id_from_path() {
    let path = PathBuf::from("/home/user/test.txt");
    let location = Location::local(path.clone());

    let same_location = Location::local(path);
    assert_eq!(location.id(), same_location.id());
}

#[test]
fn test_location_id_from_non_utf8_path_is_stable() {
    let path = non_utf8_path();
    assert!(path.to_str().is_none());

    let location = Location::local(path.clone());
    let same_location = Location::local(path);

    assert_eq!(location.id(), same_location.id());
}

#[test]
fn test_file_node_from_metadata_accepts_non_utf8_path() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let metadata = std::fs::metadata(source).unwrap();
    let path = non_utf8_path();

    let node = FileNode::from_metadata(metadata, path.clone(), None).unwrap();
    let registry = NodeRegistry::new();
    let entry = NodeEntry::from_file_node(node, &registry);

    assert_eq!(
        entry.location.descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn test_file_node_from_dir_entry_accepts_non_utf8_path() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let file_type = std::fs::metadata(source).unwrap().file_type();
    let path = non_utf8_path();

    let node = FileNode::from_dir_entry(path.clone(), file_type, None);
    let registry = NodeRegistry::new();
    let entry = NodeEntry::from_file_node(node, &registry);

    assert_eq!(
        entry.location.descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn test_location_id_different_paths() {
    let path1 = PathBuf::from("/home/user/test.txt");
    let path2 = PathBuf::from("/home/user/other.txt");

    let first = Location::local(path1);
    let second = Location::local(path2);

    assert_ne!(first.id(), second.id());
}

#[test]
fn test_file_node_is_dir() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let nonexistent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("no.md");
    let f1 = FileNode::from_path(dir, None);
    let f2 = FileNode::from_path(file, None);
    let f3 = FileNode::from_path(nonexistent, None);
    assert_eq!(f1.is_ok(), true);
    assert_eq!(f2.is_ok(), true);
    assert_eq!(f3.is_ok(), false);
    let u1 = f1.unwrap();
    let u2 = f2.unwrap();
    assert_eq!(u1.is_dir(), true);
    assert_eq!(u2.is_file(), true);
}

#[test]
fn test_file_node_extension() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let f1 = FileNode::from_path(dir, None).unwrap();
    let f2 = FileNode::from_path(file, None).unwrap();
    assert_eq!(f1.extension(), None);
    assert_eq!(f2.extension(), Some("toml"));
}

// NodeRegistry tests

#[test]
fn test_registry_new() {
    let registry = NodeRegistry::new();
    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
}

#[test]
fn test_registry_default() {
    let registry = NodeRegistry::default();
    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
}

#[test]
fn test_registry_register() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");

    let handle = registry.clone().register(path.clone());

    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    assert_eq!(
        registry.resolve_node_location(handle).unwrap().descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn test_registry_register_same_path_twice() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");

    let first = registry.clone().register(path.clone());
    let second = registry.clone().register(path);

    assert_eq!(
        registry.resolve_node_location(first),
        registry.resolve_node_location(second)
    );
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_registry_register_different_paths() {
    let registry = NodeRegistry::new();
    let path1 = PathBuf::from("/home/user/test.txt");
    let path2 = PathBuf::from("/home/user/other.txt");

    let first = registry.clone().register(path1.clone());
    let second = registry.clone().register(path2.clone());

    assert_ne!(
        registry.resolve_node_location(first),
        registry.resolve_node_location(second)
    );
    assert_eq!(registry.len(), 2);
}

#[test]
fn test_registry_resolve() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");

    let handle = registry.clone().register(path.clone());
    let resolved = registry.resolve_node_location(handle);

    assert_eq!(
        resolved.unwrap().descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn test_registry_resolve_not_found() {
    let registry = NodeRegistry::new();
    let unregistered = Location::local("/home/user/test.txt");

    let resolved = registry.resolve_location(unregistered.id());
    assert_eq!(resolved, None);
}

#[test]
fn test_registry_get_id() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");

    let handle = registry.clone().register(path.clone());
    let found_id = registry.get_id(&path);

    assert_eq!(
        found_id.and_then(|id| registry.resolve_node_location(id)),
        registry.resolve_node_location(handle)
    );
}

#[test]
fn test_registry_get_id_not_registered() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");

    let found_id = registry.get_id(&path);
    assert_eq!(found_id, None);
}

#[test]
fn test_registry_unregister() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");

    let handle = registry.clone().register(path.clone());
    assert_eq!(registry.len(), 1);

    let removed = registry.unregister(handle);

    assert_eq!(removed, Some(path));
    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
    assert_eq!(registry.resolve_node_location(handle), None);
}

#[test]
fn test_registry_unregister_not_found() {
    let registry = NodeRegistry::new();
    let handle = registry
        .register_location_node(Location::local("/home/user/test.txt"))
        .unwrap();
    registry.unregister(handle);

    let removed = registry.unregister(handle);
    assert_eq!(removed, None);
}

#[test]
fn test_registry_clear() {
    let registry = NodeRegistry::new();

    registry
        .clone()
        .register(PathBuf::from("/home/user/test1.txt"));
    registry
        .clone()
        .register(PathBuf::from("/home/user/test2.txt"));
    registry
        .clone()
        .register(PathBuf::from("/home/user/test3.txt"));

    assert_eq!(registry.len(), 3);

    registry.clear();

    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
}

#[test]
fn test_registry_register_batch() {
    let registry = NodeRegistry::new();
    let paths = vec![
        PathBuf::from("/home/user/test1.txt"),
        PathBuf::from("/home/user/test2.txt"),
        PathBuf::from("/home/user/test3.txt"),
    ];

    let ids = registry.clone().register_batch(&paths);

    assert_eq!(ids.len(), 3);
    assert_eq!(registry.len(), 3);

    for (path, id) in paths.iter().zip(ids.iter()) {
        assert_eq!(
            registry.resolve_node_location(*id).unwrap().descriptor(),
            Some(&LocationDescriptor::local(path.clone()))
        );
    }
}

#[test]
fn test_registry_register_batch_empty() {
    let registry = NodeRegistry::new();
    let paths: Vec<PathBuf> = vec![];

    let ids = registry.clone().register_batch(&paths);

    assert_eq!(ids.len(), 0);
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_resolve_batch() {
    let registry = NodeRegistry::new();
    let paths = vec![
        PathBuf::from("/home/user/test1.txt"),
        PathBuf::from("/home/user/test2.txt"),
        PathBuf::from("/home/user/test3.txt"),
    ];

    let ids = registry.clone().register_batch(&paths);
    let resolved = registry.resolve_batch(&ids);

    assert_eq!(resolved.len(), 3);
    for (path, resolved_path) in paths.iter().zip(resolved.iter()) {
        let location = resolved_path
            .as_ref()
            .map(|path| LocationDescriptor::local(path.clone()));
        assert_eq!(
            location.as_ref(),
            Some(&LocationDescriptor::local(path.clone()))
        );
    }
}

#[test]
fn test_registry_resolve_batch_mixed() {
    let registry = NodeRegistry::new();
    let path1 = PathBuf::from("/home/user/test1.txt");
    let path2 = PathBuf::from("/home/user/test2.txt");
    let path3 = PathBuf::from("/home/user/test3.txt");

    let first = registry.clone().register(path1.clone());
    let unregistered = registry
        .clone()
        .register_location_node(Location::local(path2.clone()))
        .unwrap();
    registry.unregister(unregistered);
    let third = registry.clone().register(path3.clone());

    let resolved = registry.resolve_batch(&[first, unregistered, third]);

    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved[0], Some(path1));
    assert_eq!(resolved[1], None);
    assert_eq!(resolved[2], Some(path3));
}

#[test]
fn test_registry_resolve_batch_empty() {
    let registry = NodeRegistry::new();
    let resolved = registry.resolve_batch(&[]);

    assert_eq!(resolved.len(), 0);
}

#[test]
fn test_registry_multiple_operations() {
    let registry = NodeRegistry::new();

    let path1 = PathBuf::from("/home/user/test1.txt");
    let path2 = PathBuf::from("/home/user/test2.txt");
    let first = registry.clone().register(path1.clone());
    let second = registry.clone().register(path2.clone());

    assert_eq!(registry.len(), 2);
    assert_eq!(
        registry.resolve_node_location(first).unwrap().descriptor(),
        Some(&LocationDescriptor::local(path1))
    );
    assert_eq!(
        registry.resolve_node_location(second).unwrap().descriptor(),
        Some(&LocationDescriptor::local(path2.clone()))
    );

    registry.unregister(first);
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.resolve_node_location(first), None);
    assert_eq!(
        registry.resolve_node_location(second).unwrap().descriptor(),
        Some(&LocationDescriptor::local(path2))
    );

    let path3 = PathBuf::from("/home/user/test3.txt");
    let third = registry.clone().register(path3);
    assert_eq!(registry.len(), 2);

    registry.clear();
    assert_eq!(registry.len(), 0);
    assert_eq!(registry.resolve_node_location(second), None);
    assert_eq!(registry.resolve_node_location(third), None);
}

#[test]
fn test_registry_deterministic_location_refs() {
    let registry1 = NodeRegistry::new();
    let registry2 = NodeRegistry::new();

    let path = PathBuf::from("/home/user/test.txt");

    let first = registry1.clone().register(path.clone());
    let second = registry2.clone().register(path);

    assert_eq!(
        registry1.resolve_node_location(first).unwrap(),
        registry2.resolve_node_location(second).unwrap()
    );
}

#[test]
// Compatibility pin for API-008: preserve deterministic NodeId generation
// until the identity type is removed.
fn test_registry_deterministic_ids() {
    let registry1 = NodeRegistry::new();
    let registry2 = NodeRegistry::new();

    let path = PathBuf::from("/home/user/test.txt");

    let first = registry1.clone().register(path.clone());
    let second = registry2.clone().register(path);

    assert_eq!(first, second);
}
