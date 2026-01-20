//! Tests for model layer

use crate::model::node::{FileNode, NodeId, NodeKind};
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
    todo!()
}

#[test]
fn test_file_node_is_file() {
    todo!()
}

#[test]
fn test_file_node_extension() {
    todo!()
}
