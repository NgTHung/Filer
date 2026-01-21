//! Tests for VFS providers

use crate::vfs::provider::{Capabilities, FsProvider};
use crate::vfs::local::LocalFs;
use std::path::PathBuf;

#[tokio::test]
async fn test_local_fs_list() {
    todo!()
}

#[tokio::test]
async fn test_local_fs_read() {
    todo!()
}

#[tokio::test]
async fn test_local_fs_read_range() {
    todo!()
}

#[tokio::test]
async fn test_local_fs_exists() {
    todo!()
}

#[tokio::test]
async fn test_local_fs_metadata() {
    todo!()
}

#[tokio::test]
async fn test_local_fs_capabilities() {
    todo!()
}

// Mock filesystem for testing
pub struct MockFs {
    files: std::collections::HashMap<PathBuf, Vec<u8>>,
}

impl MockFs {
    pub fn new() -> Self {
        Self {
            files: std::collections::HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: PathBuf, content: Vec<u8>) {
        self.files.insert(path, content);
    }
}
