//! Tests for VFS providers

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::model::registry::NodeRegistry;
use crate::vfs::local::LocalFs;
use crate::vfs::provider::{Capabilities, FsProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ===== LocalFs Tests =====

#[tokio::test]
async fn test_local_fs_scheme() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    assert_eq!(fs.scheme(), "file");
}

#[tokio::test]
async fn test_local_fs_capabilities() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let caps = fs.capabilities();

    assert_eq!(caps.read, true);
    assert_eq!(caps.write, true);
    assert_eq!(caps.watch, true);
    assert_eq!(caps.search, false);
}

#[tokio::test]
async fn test_local_fs_list() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Test listing the filer-core/src directory
    let result = fs.list(Path::new("/home/bbq/Documents/filer/filer-core/src")).await;
    assert!(result.is_ok());

    let files = result.unwrap();
    assert!(files.len() > 0);

    // Should contain lib.rs
    assert!(files.iter().any(|f| f.name == "lib.rs"));
}

#[tokio::test]
async fn test_local_fs_list_empty_directory() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Create a temporary empty directory
    let temp_dir = std::env::temp_dir().join("filer_test_empty");
    let _ = std::fs::create_dir(&temp_dir);

    let result = fs.list(&temp_dir).await;
    assert!(result.is_ok());

    let files = result.unwrap();
    assert_eq!(files.len(), 0);

    // Cleanup
    let _ = std::fs::remove_dir(&temp_dir);
}

#[tokio::test]
async fn test_local_fs_list_not_found() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let result = fs.list(Path::new("/nonexistent/directory/path")).await;

    assert!(result.is_err());
    match result {
        Err(CoreError::NotFound(_)) => {}
        _ => panic!("Expected NotFound error"),
    }
}

#[tokio::test]
async fn test_local_fs_read() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Create a temporary file
    let temp_file = std::env::temp_dir().join("filer_test_read.txt");
    let content = b"Hello, World!";
    std::fs::write(&temp_file, content).unwrap();

    let result = fs.read(&temp_file).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data, content);

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[tokio::test]
async fn test_local_fs_read_not_found() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let result = fs.read(Path::new("/nonexistent/file.txt")).await;

    assert!(result.is_err());
    match result {
        Err(CoreError::NotFound(_)) => {}
        _ => panic!("Expected NotFound error"),
    }
}

#[tokio::test]
async fn test_local_fs_read_range() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Create a temporary file
    let temp_file = std::env::temp_dir().join("filer_test_read_range.txt");
    let content = b"0123456789ABCDEFGHIJ";
    std::fs::write(&temp_file, content).unwrap();

    // Read bytes 5-9 (5 bytes starting at position 5)
    let result = fs.read_range(&temp_file, 5, 5).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data, b"56789");

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[tokio::test]
async fn test_local_fs_read_range_full() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Create a temporary file
    let temp_file = std::env::temp_dir().join("filer_test_read_range_full.txt");
    let content = b"Hello, World!";
    std::fs::write(&temp_file, content).unwrap();

    // Read entire file
    let result = fs.read_range(&temp_file, 0, content.len() as u64).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data, content);

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[tokio::test]
async fn test_local_fs_read_range_beyond_end() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Create a temporary file
    let temp_file = std::env::temp_dir().join("filer_test_read_range_beyond.txt");
    let content = b"Hello";
    std::fs::write(&temp_file, content).unwrap();

    // Try to read beyond file size
    let result = fs.read_range(&temp_file, 0, 100).await;
    assert!(result.is_ok());

    // Should return only available content
    let data = result.unwrap();
    assert_eq!(data, content);

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[tokio::test]
async fn test_local_fs_exists() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Test existing file
    let temp_file = std::env::temp_dir().join("filer_test_exists.txt");
    std::fs::write(&temp_file, b"test").unwrap();

    assert_eq!(fs.exists(&temp_file).await.unwrap(), true);

    // Cleanup and test non-existing file
    std::fs::remove_file(&temp_file).unwrap();
    assert_eq!(fs.exists(&temp_file).await.unwrap(), false);
}

#[tokio::test]
async fn test_local_fs_exists_directory() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    let temp_dir = std::env::temp_dir().join("filer_test_exists_dir");
    std::fs::create_dir(&temp_dir).unwrap();

    assert_eq!(fs.exists(&temp_dir).await.unwrap(), true);

    std::fs::remove_dir(&temp_dir).unwrap();
    assert_eq!(fs.exists(&temp_dir).await.unwrap(), false);
}

#[tokio::test]
async fn test_local_fs_metadata() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Create a temporary file
    let temp_file = std::env::temp_dir().join("filer_test_metadata.txt");
    let content = b"Hello, World!";
    std::fs::write(&temp_file, content).unwrap();

    let result = fs.metadata(&temp_file).await;
    assert!(result.is_ok());

    let node = result.unwrap();
    assert_eq!(node.name, "filer_test_metadata.txt");
    assert_eq!(node.size, content.len() as u64);
    assert!(node.is_file());
    assert_eq!(node.extension(), Some("txt"));

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
}

#[tokio::test]
async fn test_local_fs_metadata_directory() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    let temp_dir = std::env::temp_dir().join("filer_test_metadata_dir");
    std::fs::create_dir(&temp_dir).unwrap();

    let result = fs.metadata(&temp_dir).await;
    assert!(result.is_ok());

    let node = result.unwrap();
    assert_eq!(node.name, "filer_test_metadata_dir");
    assert!(node.is_dir());

    // Cleanup
    let _ = std::fs::remove_dir(&temp_dir);
}

#[tokio::test]
async fn test_local_fs_metadata_not_found() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let result = fs.metadata(Path::new("/nonexistent/file.txt")).await;

    assert!(result.is_err());
    match result {
        Err(CoreError::NotFound(_)) => {}
        _ => panic!("Expected NotFound error"),
    }
}

// ===== MockFs Implementation =====

pub struct MockFs {
    files: HashMap<PathBuf, Vec<u8>>,
    directories: Vec<PathBuf>,
}

impl MockFs {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            directories: Vec::new(),
        }
    }

    pub fn add_file(&mut self, path: PathBuf, content: Vec<u8>) {
        self.files.insert(path, content);
    }

    // pub fn add_directory(&mut self, path: PathBuf) {
    //     self.directories.push(path);
    // }
}

#[async_trait]
impl FsProvider for MockFs {
    fn scheme(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: true,
            watch: false,
            search: true,
        }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        if !self.directories.contains(&path.to_path_buf()) {
            return Err(CoreError::NotFound(path.to_path_buf()));
        }

        let mut nodes = Vec::new();

        for (file_path, content) in &self.files {
            if let Some(parent) = file_path.parent() {
                if parent == path {
                    nodes.push(FileNode::from_path(file_path.clone(), None)?);
                }
            }
        }

        for dir_path in &self.directories {
            if let Some(parent) = dir_path.parent() {
                if parent == path && dir_path != &path.to_path_buf() {
                    nodes.push(FileNode::from_path(dir_path.clone(), None)?);
                }
            }
        }

        Ok(nodes)
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, CoreError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(path.to_path_buf()))
    }

    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError> {
        let content = self.read(path).await?;
        let start = start as usize;
        let end = (start + len as usize).min(content.len());

        if start >= content.len() {
            return Ok(Vec::new());
        }

        Ok(content[start..end].to_vec())
    }

    async fn exists(&self, path: &Path) -> Result<bool, CoreError> {
        Ok(self.files.contains_key(path) || self.directories.contains(&path.to_path_buf()))
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        if self.files.contains_key(path) || self.directories.contains(&path.to_path_buf()) {
            FileNode::from_path(path.to_path_buf(), None)
        } else {
            Err(CoreError::NotFound(path.to_path_buf()))
        }
    }
}

// ===== MockFs Tests =====

#[tokio::test]
async fn test_mock_fs_scheme() {
    let fs = MockFs::new();
    assert_eq!(fs.scheme(), "mock");
}

#[tokio::test]
async fn test_mock_fs_capabilities() {
    let fs = MockFs::new();
    let caps = fs.capabilities();

    assert_eq!(caps.read, true);
    assert_eq!(caps.write, true);
    assert_eq!(caps.watch, false);
    assert_eq!(caps.search, true);
}

#[tokio::test]
async fn test_mock_fs_read() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");
    let content = b"Hello, MockFs!".to_vec();

    fs.add_file(path.clone(), content.clone());

    let result = fs.read(&path).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), content);
}

#[tokio::test]
async fn test_mock_fs_read_not_found() {
    let fs = MockFs::new();
    let result = fs.read(Path::new("/nonexistent.txt")).await;

    assert!(result.is_err());
    match result {
        Err(CoreError::NotFound(_)) => {}
        _ => panic!("Expected NotFound error"),
    }
}

#[tokio::test]
async fn test_mock_fs_read_range() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");
    let content = b"0123456789".to_vec();

    fs.add_file(path.clone(), content);

    let result = fs.read_range(&path, 3, 4).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"3456");
}

#[tokio::test]
async fn test_mock_fs_exists() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");

    assert_eq!(fs.exists(&path).await.unwrap(), false);

    fs.add_file(path.clone(), b"test".to_vec());
    assert_eq!(fs.exists(&path).await.unwrap(), true);
}

#[tokio::test]
async fn test_mock_fs_trait_usage() {
    // Test that MockFs can be used through the FsProvider trait
    let mut fs = MockFs::new();
    fs.add_file(PathBuf::from("/test.txt"), b"content".to_vec());

    let provider: &dyn FsProvider = &fs;
    assert_eq!(provider.scheme(), "mock");

    let result = provider.read(Path::new("/test.txt")).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"content");
}
