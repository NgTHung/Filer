//! Tests for model layer

use crate::model::node::{FileNode, NodeId};
use crate::model::registry::NodeRegistry;
use std::path::PathBuf;

#[test]
fn test_node_id_from_path() {
    let path = PathBuf::from("/home/user/test.txt");
    let id = NodeId::from_path(&path);
    
    // Same path should produce same ID
    let id2 = NodeId::from_path(&path);
    assert_eq!(id, id2);
}

#[test]
fn test_node_id_different_paths() {
    let path1 = PathBuf::from("/home/user/test.txt");
    let path2 = PathBuf::from("/home/user/other.txt");
    
    let id1 = NodeId::from_path(&path1);
    let id2 = NodeId::from_path(&path2);
    
    assert_ne!(id1, id2);
}

#[test]
fn test_file_node_is_dir() {
    let f1 = FileNode::from_path(PathBuf::from("~/Documents/filer"),None);
    let f2 = FileNode::from_path(PathBuf::from("~/Documents/filer/README.md"),None);
    let f3 = FileNode::from_path(PathBuf::from("~/Documents/filer/no.md"),None);
    assert_eq!(f1.is_ok(),true);
    assert_eq!(f2.is_ok(),true);
    assert_eq!(f3.is_ok(),false);
    let u1 = f1.unwrap();
    let u2 = f2.unwrap();
    assert_eq!(u1.is_dir(),true);
    assert_eq!(u2.is_file(), true);
}

#[test]
fn test_file_node_extension() {
    let f1 = FileNode::from_path(PathBuf::from("~/Documents/filer"),None).unwrap();
    let f2 = FileNode::from_path(PathBuf::from("~/Documents/filer/README.md"),None).unwrap();
    assert_eq!(f1.extension(),None);
    assert_eq!(f2.extension(),Some("md"));
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
    
    let id = registry.clone().register(path.clone());
    
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    assert_eq!(id, NodeId::from_path(&path));
}

#[test]
fn test_registry_register_same_path_twice() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");
    
    let id1 = registry.clone().register(path.clone());
    let id2 = registry.clone().register(path.clone());
    
    // Should produce same ID
    assert_eq!(id1, id2);
    // Should still have only one entry (overwrites)
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_registry_register_different_paths() {
    let registry = NodeRegistry::new();
    let path1 = PathBuf::from("/home/user/test.txt");
    let path2 = PathBuf::from("/home/user/other.txt");
    
    let id1 = registry.clone().register(path1.clone());
    let id2 = registry.clone().register(path2.clone());
    
    assert_ne!(id1, id2);
    assert_eq!(registry.len(), 2);
}

#[test]
fn test_registry_resolve() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");
    
    let id = registry.clone().register(path.clone());
    let resolved = registry.resolve(id);
    
    assert_eq!(resolved, Some(path));
}

#[test]
fn test_registry_resolve_not_found() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");
    let id = NodeId::from_path(&path);
    
    let resolved = registry.resolve(id);
    assert_eq!(resolved, None);
}

#[test]
fn test_registry_get_id() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");
    
    let id = registry.clone().register(path.clone());
    let found_id = registry.get_id(&path);
    
    assert_eq!(found_id, Some(id));
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
    
    let id = registry.clone().register(path.clone());
    assert_eq!(registry.len(), 1);
    
    let removed = registry.unregister(id);
    
    assert_eq!(removed, Some(path));
    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
    assert_eq!(registry.resolve(id), None);
}

#[test]
fn test_registry_unregister_not_found() {
    let registry = NodeRegistry::new();
    let path = PathBuf::from("/home/user/test.txt");
    let id = NodeId::from_path(&path);
    
    let removed = registry.unregister(id);
    assert_eq!(removed, None);
}

#[test]
fn test_registry_clear() {
    let registry = NodeRegistry::new();
    
    registry.clone().register(PathBuf::from("/home/user/test1.txt"));
    registry.clone().register(PathBuf::from("/home/user/test2.txt"));
    registry.clone().register(PathBuf::from("/home/user/test3.txt"));
    
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
    
    // Check each ID matches
    for (path, id) in paths.iter().zip(ids.iter()) {
        assert_eq!(*id, NodeId::from_path(path));
        assert_eq!(registry.resolve(*id), Some(path.clone()));
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
        assert_eq!(*resolved_path, Some(path.clone()));
    }
}

#[test]
fn test_registry_resolve_batch_mixed() {
    let registry = NodeRegistry::new();
    let path1 = PathBuf::from("/home/user/test1.txt");
    let path2 = PathBuf::from("/home/user/test2.txt");
    let path3 = PathBuf::from("/home/user/test3.txt");
    
    let id1 = registry.clone().register(path1.clone());
    let id2 = NodeId::from_path(&path2); // Not registered
    let id3 = registry.clone().register(path3.clone());
    
    let resolved = registry.resolve_batch(&[id1, id2, id3]);
    
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
    
    // Register some paths
    let path1 = PathBuf::from("/home/user/test1.txt");
    let path2 = PathBuf::from("/home/user/test2.txt");
    let id1 = registry.clone().register(path1.clone());
    let id2 = registry.clone().register(path2.clone());
    
    assert_eq!(registry.len(), 2);
    
    // Resolve them
    assert_eq!(registry.resolve(id1), Some(path1));
    assert_eq!(registry.resolve(id2), Some(path2.clone()));
    
    // Unregister one
    registry.unregister(id1);
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.resolve(id1), None);
    assert_eq!(registry.resolve(id2), Some(path2));
    
    // Register a new one
    let path3 = PathBuf::from("/home/user/test3.txt");
    let id3 = registry.clone().register(path3.clone());
    assert_eq!(registry.len(), 2);
    
    // Clear all
    registry.clear();
    assert_eq!(registry.len(), 0);
    assert_eq!(registry.resolve(id2), None);
    assert_eq!(registry.resolve(id3), None);
}

#[test]
fn test_registry_deterministic_ids() {
    let registry1 = NodeRegistry::new();
    let registry2 = NodeRegistry::new();
    
    let path = PathBuf::from("/home/user/test.txt");
    
    let id1 = registry1.register(path.clone());
    let id2 = registry2.register(path.clone());
    
    // Same path should produce same ID in different registries
    assert_eq!(id1, id2);
}

