use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};
use crate::{
    model::node::{NodeId, NodeKind, NodeMeta},
    utils,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

fn make_file(name: &str, path: &str, size: u64, hidden: bool) -> FileNode {
    let extension = utils::get_extension(PathBuf::from(name).as_path()).map(str::to_string);
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("{path}/{name}")),
        kind: NodeKind::File { extension },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    }
}

fn _make_file_with_ext(name: &str, path: &str, ext: Option<&str>, size: u64) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("{path}/{name}")),
        kind: NodeKind::File {
            extension: ext.map(|s| s.to_string()),
        },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    }
}

fn _make_dir(name: &str, full_path: &str, hidden: bool) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64 + 1000),
        name: name.to_string(),
        path: PathBuf::from(format!("{full_path}/{name}")),
        kind: NodeKind::Directory {
            children_count: None,
        },
        size: 0,
        modified: Some(SystemTime::UNIX_EPOCH),
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    }
}

/// Mock filesystem provider for testing Scanner behavior
#[derive(Clone)]
struct MockProvider {
    files: Arc<Mutex<Vec<FileNode>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    fn add_file(&self, node: FileNode) {
        self.files.lock().unwrap().push(node);
    }

    fn get_list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }
}

#[async_trait]
impl FsProvider for MockProvider {
    fn scheme(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::NotFound(path.to_path_buf()));
        }

        self.list_calls.lock().unwrap().push(path.to_path_buf());
        Ok(self.files.lock().unwrap().clone())
    }

    async fn read(&self, _path: &Path) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(&self, _path: &Path, _start: u64, _len: u64) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, _path: &Path) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(&self, _path: &Path) -> Result<FileNode, CoreError> {
        Err(CoreError::NotFound(PathBuf::from("test")))
    }
}

// Scanner actor integration tests moved to filer-core/tests/scanner_integration_test.rs

#[cfg(test)]
mod scanner_command_tests {
    use std::path::PathBuf;

    use crate::{modules::scan::scanner::ScanCommand, model::session};

    #[test]
    fn test_scan_command_clone() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::Scan {
            path: PathBuf::from("/test"),
            pipeline: crate::pipeline::PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session,
        };

        let cloned = cmd.clone();

        match (cmd, cloned) {
            (
                ScanCommand::Scan {
                    path: p1,
                    pipeline: pl1,
                    session: s1,
                },
                ScanCommand::Scan {
                    path: p2,
                    pipeline:pl2,
                    session: s2,
                },
            ) => {
                assert_eq!(s1, s2);
                assert_eq!(p1, p2);
                assert_eq!(pl1, pl2);
            }
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_scan_command_debug() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::Scan {
            path: PathBuf::from("/test/path"),pipeline: crate::pipeline::PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session,
        };

        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Scan"));
        assert!(debug_str.contains("/test/path"));
    }

    #[test]
    fn test_cancel_command() {
        let session = session::SessionId::new();
        let cmd = ScanCommand::Cancel(session);
        let _cloned = cmd.clone();
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Cancel"));
    }
}

#[cfg(test)]
mod mock_provider_tests {
    use super::*;

    #[test]
    fn test_mock_provider_capabilities() {
        let provider = MockProvider::new();
        let caps = provider.capabilities();

        assert!(caps.read);
        assert!(!caps.write);
        assert!(!caps.watch);
        assert!(!caps.search);
    }

    #[test]
    fn test_mock_provider_scheme() {
        let provider = MockProvider::new();
        assert_eq!(provider.scheme(), "mock");
    }

    #[tokio::test]
    async fn test_mock_provider_list_success() {
        let provider = MockProvider::new();

        // provider.add_file(FileNode {
        //     id: 1.into(),
        //     name: "test.txt".to_string(),
        //     path: PathBuf::from("/test.txt"),
        //     is_dir: false,
        //     size: 100,
        //     modified: None,
        //     created: None,
        //     permissions: None,
        //     metadata: None,
        // });
        provider.add_file(make_file("test.txt", "/test", 100, false));

        let result = provider.list(Path::new("/test")).await;

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "test.txt");
    }

    #[tokio::test]
    async fn test_mock_provider_tracks_calls() {
        let provider = MockProvider::new();

        provider.list(Path::new("/dir1")).await.unwrap();
        provider.list(Path::new("/dir2")).await.unwrap();

        let calls = provider.get_list_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], PathBuf::from("/dir1"));
        assert_eq!(calls[1], PathBuf::from("/dir2"));
    }

    #[tokio::test]
    async fn test_mock_provider_can_fail() {
        let provider = MockProvider::new();
        provider.set_should_fail(true);

        let result = provider.list(Path::new("/test")).await;
        assert!(result.is_err());
    }
}
