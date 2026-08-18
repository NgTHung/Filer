// Tests for VFS providers

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
async fn test_local_fs_list_emits_reconstructable_locations() {
    let (fs, dir) = local_fs();
    let path = dir.path().join("entry.txt");
    std::fs::write(&path, b"entry").unwrap();

    let entries = fs
        .list(dir.path(), &crate::ProviderCx::none())
        .await
        .unwrap();
    let entry = entries.iter().find(|entry| entry.name == "entry.txt").unwrap();

    assert_eq!(
        entry.location.descriptor(),
        Some(&crate::LocationDescriptor::local(path))
    );
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
    assert!(page.entries.iter().all(|entry| entry.location.descriptor().is_some()));
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
    assert_eq!(
        node.location.descriptor(),
        Some(&crate::LocationDescriptor::local(temp_file))
    );
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
