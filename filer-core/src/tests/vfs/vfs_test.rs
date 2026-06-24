//! Tests for VFS providers

use crate::errors::{CoreError, ErrorCode};
use crate::model::directory::{DirectoryCursor, DirectoryPageRequest};
use crate::model::node::FileNode;
use crate::model::registry::NodeRegistry;
use crate::services::mime::{DetectionConfidence, MAGIC_BYTE_WINDOW, MimeCategory, MimeDetector};
use crate::vfs::local::LocalFs;
use crate::vfs::provider::{Capabilities, FsProvider, ListingOptions};
use crate::vfs::segmented::SegmentedLocationResolver;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn local_fs() -> (LocalFs, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let fs = LocalFs::new(NodeRegistry::new());
    (fs, dir)
}

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

    assert!(caps.read);
    assert!(caps.write);
    assert!(caps.watch);
    assert!(!caps.search);
}

#[tokio::test]
async fn test_local_fs_list() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);

    // Test listing the filer-core/src directory
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let result = fs.list(&src_dir, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());

    let files = result.unwrap();
    assert!(!files.is_empty());

    // Should contain lib.rs
    assert!(files.iter().any(|f| f.name == "lib.rs"));
}

#[tokio::test]
async fn test_local_fs_list_empty_directory() {
    let (fs, dir) = local_fs();

    let result = fs.list(dir.path(), &crate::ProviderCx::none()).await;
    assert!(result.is_ok());

    let files = result.unwrap();
    assert_eq!(files.len(), 0);
}

#[tokio::test]
async fn test_local_fs_fast_listing_omits_stat_metadata() {
    let (fs, dir) = local_fs();
    let path = dir.path().join("fast.txt");
    std::fs::write(&path, b"hello").unwrap();

    let nodes = fs
        .list_with_options(
            dir.path(),
            ListingOptions::fast(),
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();
    let node = nodes.iter().find(|node| node.name == "fast.txt").unwrap();

    assert_eq!(node.size, 0);
    assert!(node.modified.is_none());
    assert!(node.created.is_none());
    assert!(node.accessed.is_none());
}

#[tokio::test]
async fn test_local_fs_metadata_listing_populates_stat_metadata() {
    let (fs, dir) = local_fs();
    let path = dir.path().join("metadata.txt");
    std::fs::write(&path, b"hello").unwrap();

    let nodes = fs
        .list_with_options(
            dir.path(),
            ListingOptions::metadata(),
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();
    let node = nodes
        .iter()
        .find(|node| node.name == "metadata.txt")
        .unwrap();

    assert_eq!(node.size, 5);
    assert!(node.modified.is_some());
}

#[tokio::test]
async fn test_local_fs_list_with_meta_matches_metadata_listing() {
    let (fs, dir) = local_fs();
    let path = dir.path().join("compat.txt");
    std::fs::write(&path, b"hello").unwrap();

    let from_options = fs
        .list_with_options(
            dir.path(),
            ListingOptions::metadata(),
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();
    let from_compat = fs
        .list_with_meta(dir.path(), &crate::ProviderCx::none())
        .await
        .unwrap();

    let option_node = from_options
        .iter()
        .find(|node| node.name == "compat.txt")
        .unwrap();
    let compat_node = from_compat
        .iter()
        .find(|node| node.name == "compat.txt")
        .unwrap();
    assert_eq!(option_node.size, compat_node.size);
    assert_eq!(
        option_node.modified.is_some(),
        compat_node.modified.is_some()
    );
}

#[tokio::test]
async fn test_local_fs_list_page_returns_limit_and_cursor() {
    let (fs, dir) = local_fs();
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(dir.path().join(name), b"hello").unwrap();
    }

    let page = fs
        .list_page(
            dir.path(),
            DirectoryPageRequest {
                listing: ListingOptions::fast(),
                limit: 2,
                cursor: None,
            },
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();

    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.state.page_count, 2);
    assert_eq!(page.state.total_count, None);
    assert!(page.state.next_cursor.is_some());
    assert!(!page.state.complete);
}

#[tokio::test]
async fn test_local_fs_list_page_cursor_returns_later_entries() {
    let (fs, dir) = local_fs();
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(dir.path().join(name), b"hello").unwrap();
    }

    let first = fs
        .list_page(
            dir.path(),
            DirectoryPageRequest {
                listing: ListingOptions::fast(),
                limit: 2,
                cursor: None,
            },
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();
    let second = fs
        .list_page(
            dir.path(),
            DirectoryPageRequest {
                listing: ListingOptions::fast(),
                limit: 2,
                cursor: first.state.next_cursor,
            },
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();

    assert_eq!(second.entries.len(), 1);
    assert_eq!(second.state.next_cursor, None);
    assert!(second.state.complete);
}

#[tokio::test]
async fn test_local_fs_list_page_metadata_populates_stat_metadata() {
    let (fs, dir) = local_fs();
    std::fs::write(dir.path().join("metadata.txt"), b"hello").unwrap();

    let page = fs
        .list_page(
            dir.path(),
            DirectoryPageRequest {
                listing: ListingOptions::metadata(),
                limit: 1,
                cursor: None,
            },
            &crate::ProviderCx::none(),
        )
        .await
        .unwrap();

    assert_eq!(page.entries[0].size, 5);
    assert!(page.entries[0].modified.is_some());
}

#[tokio::test]
async fn test_local_fs_list_page_rejects_invalid_cursor() {
    let (fs, dir) = local_fs();

    let result = fs
        .list_page(
            dir.path(),
            DirectoryPageRequest {
                listing: ListingOptions::fast(),
                limit: 1,
                cursor: Some(DirectoryCursor("not-a-cursor".into())),
            },
            &crate::ProviderCx::none(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_local_fs_list_page_rejects_zero_limit() {
    let (fs, dir) = local_fs();

    let result = fs
        .list_page(
            dir.path(),
            DirectoryPageRequest {
                listing: ListingOptions::fast(),
                limit: 0,
                cursor: None,
            },
            &crate::ProviderCx::none(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_local_fs_list_not_found() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let result = fs
        .list(
            Path::new("/nonexistent/directory/path"),
            &crate::ProviderCx::none(),
        )
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
}

#[tokio::test]
async fn test_local_fs_read() {
    let (fs, dir) = local_fs();

    // Create a temporary file
    let temp_file = dir.path().join("filer_test_read.txt");
    let content = b"Hello, World!";
    std::fs::write(&temp_file, content).unwrap();

    let result = fs.read(&temp_file, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data, content);
}

#[tokio::test]
async fn test_local_fs_read_not_found() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let result = fs
        .read(
            Path::new("/nonexistent/file.txt"),
            &crate::ProviderCx::none(),
        )
        .await;

    assert!(result.is_err());
    print!("{:?}", result);
    assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
}

#[tokio::test]
async fn test_local_fs_read_range() {
    let (fs, dir) = local_fs();

    // Create a temporary file
    let temp_file = dir.path().join("filer_test_read_range.txt");
    let content = b"0123456789ABCDEFGHIJ";
    std::fs::write(&temp_file, content).unwrap();

    // Read bytes 5-9 (5 bytes starting at position 5)
    let result = fs
        .read_range(&temp_file, 5, 5, &crate::ProviderCx::none())
        .await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data, b"56789");
}

#[tokio::test]
async fn test_local_fs_read_range_full() {
    let (fs, dir) = local_fs();

    // Create a temporary file
    let temp_file = dir.path().join("filer_test_read_range_full.txt");
    let content = b"Hello, World!";
    std::fs::write(&temp_file, content).unwrap();

    // Read entire file
    let result = fs
        .read_range(
            &temp_file,
            0,
            content.len() as u64,
            &crate::ProviderCx::none(),
        )
        .await;
    assert!(result.is_ok());

    let data = result.unwrap();
    assert_eq!(data, content);
}

#[tokio::test]
async fn test_local_fs_read_range_beyond_end() {
    let (fs, dir) = local_fs();

    // Create a temporary file
    let temp_file = dir.path().join("filer_test_read_range_beyond.txt");
    let content = b"Hello";
    std::fs::write(&temp_file, content).unwrap();

    // Try to read beyond file size
    let result = fs
        .read_range(&temp_file, 0, 100, &crate::ProviderCx::none())
        .await;
    assert!(result.is_ok());

    // Should return only available content
    let data = result.unwrap();
    assert_eq!(data, content);
}

#[tokio::test]
async fn test_local_fs_read_header_smaller_than_window() {
    let (fs, dir) = local_fs();

    // A file smaller than the window must return the bytes it has, not error.
    let temp_file = dir.path().join("filer_test_header_small.bin");
    let content = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    std::fs::write(&temp_file, &content).unwrap();

    let result = fs
        .read_header(&temp_file, MAGIC_BYTE_WINDOW, &crate::ProviderCx::none())
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), content);
}

#[tokio::test]
async fn test_local_fs_read_header_empty_file() {
    let (fs, dir) = local_fs();

    let temp_file = dir.path().join("filer_test_header_empty.bin");
    std::fs::write(&temp_file, b"").unwrap();

    let result = fs
        .read_header(&temp_file, MAGIC_BYTE_WINDOW, &crate::ProviderCx::none())
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Vec::<u8>::new());
}

#[tokio::test]
async fn test_small_file_magic_detection() {
    let (fs, dir) = local_fs();

    // PNG signature in a file smaller than the window, with no extension so
    // detection must come from magic bytes, not the name.
    let temp_file = dir.path().join("blob");
    let png_signature = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    std::fs::write(&temp_file, &png_signature).unwrap();

    let header = fs
        .read_header(&temp_file, MAGIC_BYTE_WINDOW, &crate::ProviderCx::none())
        .await
        .unwrap();
    let info = MimeDetector::detect(&temp_file, &header);
    assert_eq!(info.category, MimeCategory::Image);
    assert_eq!(info.confidence, DetectionConfidence::Definitive);
}

#[tokio::test]
async fn test_local_fs_exists() {
    let (fs, dir) = local_fs();

    // Test existing file
    let temp_file = dir.path().join("filer_test_exists.txt");
    std::fs::write(&temp_file, b"test").unwrap();

    assert!(
        fs.exists(&temp_file, &crate::ProviderCx::none())
            .await
            .unwrap()
    );

    // Cleanup and test non-existing file
    std::fs::remove_file(&temp_file).unwrap();
    assert!(
        !fs.exists(&temp_file, &crate::ProviderCx::none())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_local_fs_exists_directory() {
    let (fs, dir) = local_fs();

    let temp_dir = dir.path().join("filer_test_exists_dir");
    std::fs::create_dir(&temp_dir).unwrap();

    assert!(
        fs.exists(&temp_dir, &crate::ProviderCx::none())
            .await
            .unwrap()
    );

    std::fs::remove_dir(&temp_dir).unwrap();
    assert!(
        !fs.exists(&temp_dir, &crate::ProviderCx::none())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_local_fs_metadata() {
    let (fs, dir) = local_fs();

    // Create a temporary file
    let temp_file = dir.path().join("filer_test_metadata.txt");
    let content = b"Hello, World!";
    std::fs::write(&temp_file, content).unwrap();

    let result = fs.metadata(&temp_file, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());

    let node = result.unwrap();
    assert_eq!(node.name, "filer_test_metadata.txt");
    assert_eq!(node.size, content.len() as u64);
    assert!(node.is_file());
    assert_eq!(node.extension(), Some("txt"));
}

#[tokio::test]
async fn test_local_fs_metadata_directory() {
    let (fs, dir) = local_fs();

    let temp_dir = dir.path().join("filer_test_metadata_dir");
    std::fs::create_dir(&temp_dir).unwrap();

    let result = fs.metadata(&temp_dir, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());

    let node = result.unwrap();
    assert_eq!(node.name, "filer_test_metadata_dir");
    assert!(node.is_dir());
}

#[tokio::test]
async fn test_local_fs_metadata_not_found() {
    let reg = NodeRegistry::new();
    let fs = LocalFs::new(reg);
    let result = fs
        .metadata(
            Path::new("/nonexistent/file.txt"),
            &crate::ProviderCx::none(),
        )
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
}

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

    async fn list(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<FileNode>, CoreError> {
        if !self.directories.contains(&path.to_path_buf()) {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let mut nodes = Vec::new();

        for file_path in self.files.keys() {
            if let Some(parent) = file_path.parent()
                && parent == path
            {
                nodes.push(FileNode::from_path(file_path.clone(), None)?);
            }
        }

        for dir_path in &self.directories {
            if let Some(parent) = dir_path.parent()
                && parent == path
                && dir_path != &path.to_path_buf()
            {
                nodes.push(FileNode::from_path(dir_path.clone(), None)?);
            }
        }

        Ok(nodes)
    }

    async fn read(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| CoreError::not_found(path.to_path_buf()))
    }

    async fn read_range(
        &self,
        path: &Path,
        start: u64,
        len: u64,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        let content = self.read(path, cx).await?;
        let start = start as usize;
        let end = (start + len as usize).min(content.len());

        if start >= content.len() {
            return Ok(Vec::new());
        }

        Ok(content[start..end].to_vec())
    }

    async fn exists(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(self.files.contains_key(path) || self.directories.contains(&path.to_path_buf()))
    }

    async fn metadata(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<FileNode, CoreError> {
        if self.files.contains_key(path) || self.directories.contains(&path.to_path_buf()) {
            FileNode::from_path(path.to_path_buf(), None)
        } else {
            Err(CoreError::not_found(path.to_path_buf()))
        }
    }
}

#[tokio::test]
async fn test_mock_fs_scheme() {
    let fs = MockFs::new();
    assert_eq!(fs.scheme(), "mock");
}

#[tokio::test]
async fn test_mock_fs_capabilities() {
    let fs = MockFs::new();
    let caps = fs.capabilities();

    assert!(caps.read);
    assert!(caps.write);
    assert!(!caps.watch);
    assert!(caps.search);
}

#[tokio::test]
async fn test_mock_fs_read() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");
    let content = b"Hello, MockFs!".to_vec();

    fs.add_file(path.clone(), content.clone());

    let result = fs.read(&path, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), content);
}

#[tokio::test]
async fn test_mock_fs_read_not_found() {
    let fs = MockFs::new();
    let result = fs
        .read(Path::new("/nonexistent.txt"), &crate::ProviderCx::none())
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
}

#[tokio::test]
async fn test_mock_fs_read_range() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");
    let content = b"0123456789".to_vec();

    fs.add_file(path.clone(), content);

    let result = fs.read_range(&path, 3, 4, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"3456");
}

#[tokio::test]
async fn test_mock_fs_exists() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");

    assert!(!fs.exists(&path, &crate::ProviderCx::none()).await.unwrap());

    fs.add_file(path.clone(), b"test".to_vec());
    assert!(fs.exists(&path, &crate::ProviderCx::none()).await.unwrap());
}

#[tokio::test]
async fn test_mock_fs_trait_usage() {
    // Test that MockFs can be used through the FsProvider trait
    let mut fs = MockFs::new();
    fs.add_file(PathBuf::from("/test.txt"), b"content".to_vec());

    let provider: &dyn FsProvider = &fs;
    assert_eq!(provider.scheme(), "mock");

    let result = provider
        .read(Path::new("/test.txt"), &crate::ProviderCx::none())
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"content");
}

mod write_tests {
    use super::*;

    fn fs() -> (LocalFs, TempDir) {
        local_fs()
    }

    #[tokio::test]
    async fn test_write_creates_new_file() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");

        fs.write(&path, b"hello", &crate::ProviderCx::none())
            .await
            .unwrap();

        let content = tokio::fs::read(&path).await.unwrap();
        assert_eq!(content, b"hello");
    }

    #[tokio::test]
    async fn test_write_overwrites_existing_file() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");

        fs.write(&path, b"first", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.write(&path, b"second", &crate::ProviderCx::none())
            .await
            .unwrap();

        let content = tokio::fs::read(&path).await.unwrap();
        assert_eq!(content, b"second");
    }

    #[tokio::test]
    async fn test_write_returns_err_for_nonexistent_parent() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("nonexistent").join("file.txt");

        let result = fs.write(&path, b"data", &crate::ProviderCx::none()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_copy_file_creates_destination() {
        let (fs, dir) = fs();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");

        fs.write(&src, b"data", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.copy(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert!(dst.exists());
    }

    #[tokio::test]
    async fn test_copy_file_preserves_content() {
        let (fs, dir) = fs();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");

        fs.write(&src, b"exact content", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.copy(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        let content = tokio::fs::read(&dst).await.unwrap();
        assert_eq!(content, b"exact content");
    }

    #[tokio::test]
    async fn test_copy_nonexistent_src_returns_err() {
        let (fs, dir) = fs();
        let src = dir.path().join("nonexistent.txt");
        let dst = dir.path().join("dst.txt");

        let result = fs.copy(&src, &dst, &crate::ProviderCx::none()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_file() {
        let (fs, dir) = fs();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");

        fs.write(&src, b"x", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.rename(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert!(dst.exists());
        assert!(!src.exists());
    }

    #[tokio::test]
    async fn test_rename_directory() {
        let (fs, dir) = fs();
        let src = dir.path().join("dir_a");
        let dst = dir.path().join("dir_b");

        tokio::fs::create_dir(&src).await.unwrap();
        fs.rename(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert!(dst.exists());
        assert!(!src.exists());
    }

    #[tokio::test]
    async fn test_rename_nonexistent_returns_err() {
        let (fs, dir) = fs();
        let src = dir.path().join("nonexistent");
        let dst = dir.path().join("dst");

        let result = fs.rename(&src, &dst, &crate::ProviderCx::none()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_file() {
        let (fs, dir) = fs();
        let path = dir.path().join("file.txt");

        fs.write(&path, b"data", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.delete(&path, &crate::ProviderCx::none()).await.unwrap();

        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_delete_directory_recursively() {
        let (fs, dir) = fs();
        let root = dir.path().join("root");

        tokio::fs::create_dir(&root).await.unwrap();
        tokio::fs::create_dir(root.join("sub")).await.unwrap();
        tokio::fs::write(root.join("sub").join("file.txt"), b"data")
            .await
            .unwrap();

        fs.delete(&root, &crate::ProviderCx::none()).await.unwrap();

        assert!(!root.exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_err() {
        let (fs, dir) = fs();
        let path = dir.path().join("nonexistent.txt");

        let result = fs.delete(&path, &crate::ProviderCx::none()).await;
        assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
    }

    #[tokio::test]
    async fn test_mkdir_creates_directory() {
        let (fs, dir) = fs();
        let path = dir.path().join("new_dir");

        fs.mkdir(&path, &crate::ProviderCx::none()).await.unwrap();

        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[tokio::test]
    async fn test_mkdir_creates_nested_directories() {
        let (fs, dir) = fs();
        let path = dir.path().join("a").join("b").join("c");

        fs.mkdir(&path, &crate::ProviderCx::none()).await.unwrap();

        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[tokio::test]
    async fn test_mkdir_existing_directory_is_ok() {
        let (fs, dir) = fs();
        let path = dir.path().join("existing");

        tokio::fs::create_dir(&path).await.unwrap();
        let result = fs.mkdir(&path, &crate::ProviderCx::none()).await;

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod segmented_location_tests {
    use super::*;
    use crate::model::location::{LocationDescriptor, LocationSegment};
    use crate::model::node::NodeKind;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn nested_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn segmented_resolver_lists_zip_root_with_target_locations() {
        let (fs, dir) = local_fs();
        let archive = dir.path().join("bundle.zip");
        write_zip(
            &archive,
            &[("src/main.rs", b"fn main() {}"), ("README.md", b"readme")],
        );
        let location = LocationDescriptor::local(&archive).archive_member("");

        let entries = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        let src = entries.iter().find(|entry| entry.name == "src").unwrap();
        assert!(matches!(src.kind, NodeKind::Directory { .. }));
        assert!(src.capabilities.read);
        assert!(src.capabilities.navigate);
        assert_eq!(
            src.location.descriptor(),
            Some(&LocationDescriptor::local(&archive).archive_member("src"))
        );

        let readme = entries
            .iter()
            .find(|entry| entry.name == "README.md")
            .unwrap();
        assert!(matches!(readme.kind, NodeKind::File { .. }));
        assert!(readme.capabilities.read);
        assert!(!readme.capabilities.navigate);
        assert_eq!(
            readme.location.descriptor(),
            Some(&LocationDescriptor::local(&archive).archive_member("README.md"))
        );
    }

    #[tokio::test]
    async fn segmented_resolver_lists_nested_zip_layers_in_order() {
        let (fs, dir) = local_fs();
        let archive = dir.path().join("outer.zip");
        let inner = nested_zip_bytes(&[("inner.txt", b"inside")]);
        write_zip(&archive, &[("nested.zip", &inner)]);
        let location = LocationDescriptor::local(&archive)
            .archive_member("nested.zip")
            .archive_member("");

        let entries = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "inner.txt");
        assert_eq!(
            entries[0].location.descriptor(),
            Some(
                &LocationDescriptor::local(&archive)
                    .archive_member("nested.zip")
                    .archive_member("inner.txt")
            )
        );
    }

    #[tokio::test]
    async fn segmented_resolver_rejects_virtual_segments_as_structured_error() {
        let (fs, dir) = local_fs();
        let archive = dir.path().join("bundle.zip");
        write_zip(&archive, &[("README.md", b"readme")]);
        let location = LocationDescriptor::local(&archive).with_segment(LocationSegment::Virtual {
            scheme: "git".to_string(),
            path: PathBuf::from("HEAD"),
        });

        let error = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::LocationSegmentedUnsupported);
    }

    #[tokio::test]
    async fn segmented_resolver_rejects_non_local_provider_as_structured_error() {
        let fs = MockFs::new();
        let location = LocationDescriptor::provider_profile("s3", "assets", "bucket/archive.zip")
            .archive_member("");

        let error = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::UnsupportedProvider);
    }
}

#[cfg(test)]
mod context_cancellation_tests {
    use super::*;

    fn cancelled_cx() -> (crate::CancelSignal, crate::ProviderCx<'static>) {
        let cancel = crate::CancelSignal::new();
        cancel.cancel();
        let leaked = Box::leak(Box::new(cancel.clone()));
        (cancel, crate::ProviderCx::with_cancel(leaked))
    }

    #[tokio::test]
    async fn test_local_fs_list_rejects_pre_cancelled_context() {
        let (fs, dir) = local_fs();
        let (_cancel, cx) = cancelled_cx();

        let result = fs.list(dir.path(), &cx).await;

        assert_eq!(result.unwrap_err().code(), ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn test_local_fs_read_header_rejects_pre_cancelled_context() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");
        tokio::fs::write(&path, b"hello").await.unwrap();
        let (_cancel, cx) = cancelled_cx();

        let result = fs.read_header(&path, MAGIC_BYTE_WINDOW, &cx).await;

        assert_eq!(result.unwrap_err().code(), ErrorCode::Cancelled);
    }

    #[tokio::test]
    async fn test_local_fs_write_rejects_pre_cancelled_context() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");
        let (_cancel, cx) = cancelled_cx();

        let result = fs.write(&path, b"hello", &cx).await;

        assert_eq!(result.unwrap_err().code(), ErrorCode::Cancelled);
        assert!(!path.exists());
    }
}
