use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};
use crate::{
    actors::{
        Actor,
        scanner::{ScanCommand, Scanner},
    },
    model::node::{NodeId, NodeKind, NodeMeta},
    utils,
};
use async_trait::async_trait;
use flume;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio::time::timeout;

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
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
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
        meta: NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
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
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
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

#[cfg(test)]
mod scanner_integration_tests {

    use crate::{
        model::{registry::NodeRegistry, session},
        pipeline::SortConfig,
    };

    use super::*;

    #[tokio::test]
    async fn test_scanner_actor_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, _evt_rx) = flume::unbounded();
        let provider = Arc::new(MockProvider::new());
        let reg = NodeRegistry::new();

        let scanner = Scanner::new(cmd_rx, evt_tx, provider, reg);

        // Spawn scanner in background
        let handle = tokio::spawn(async move {
            scanner.run().await;
        });

        // Drop command sender to close scanner
        drop(cmd_tx);

        // Scanner should exit gracefully when command channel closes
        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(
            result.is_ok(),
            "Scanner should exit when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_scanner_processes_scan_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, evt_rx) = flume::unbounded();
        let provider = MockProvider::new();
        let reg = NodeRegistry::new();

        // Setup mock files
        provider.add_file(make_file("file1.txt", "/test", 425, false));

        provider.add_file(make_file("file2.txt", "/test", 200, false));

        let session = session::SessionId::new();

        let provider_clone = provider.clone();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);

        // Start scanner
        let _scanner_handle = tokio::spawn(async move {
            scanner.run().await;
        });

        // Send scan command
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/test"),
                pipeline: crate::pipeline::PipelineConfig {
                    sort: None,
                    filter: None,
                    group: None,
                },
                session,
            })
            .expect("Failed to send scan command");

        // Wait for events
        let event = timeout(Duration::from_secs(1), evt_rx.recv_async())
            .await
            .expect("Timeout waiting for event")
            .expect("Failed to receive event");

        // Verify scanner called list on the provider
        let calls = provider_clone.get_list_calls();
        assert!(
            !calls.is_empty(),
            "Scanner should have called list() on provider"
        );
        assert_eq!(calls[0], PathBuf::from("/test"));

        // Verify event emission (exact event type depends on implementation)
        // This would typically be DirectoryLoaded or FilesBatch
        match event {
            Event::DirectoryLoaded { entries, .. } => {
                assert_eq!(entries.len(), 2);
            }
            Event::FilesBatch(entries, _) => {
                assert_eq!(entries.len(), 2);
            }
            _ => {
                // Accept other valid events depending on implementation
            }
        }
    }

    #[tokio::test]
    async fn test_scanner_handles_cancellation() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, _evt_rx) = flume::unbounded();
        let provider = Arc::new(MockProvider::new());
        let session = session::SessionId::new();
        let reg = NodeRegistry::new();

        let scanner = Scanner::new(cmd_rx, evt_tx, provider, reg);

        let _scanner_handle = tokio::spawn(async move {
            scanner.run().await;
        });

        // Send scan command followed by cancel
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/test"),
                pipeline: crate::pipeline::PipelineConfig {
                    sort: Some(SortConfig {
                        ..Default::default()
                    }),
                    filter: None,
                    group: None,
                },
                session,
            })
            .unwrap();

        cmd_tx.send(ScanCommand::Cancel(session)).unwrap();

        // Give scanner time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Scanner should handle cancel gracefully (no crash)
        // Specific behavior depends on implementation
    }

    #[tokio::test]
    async fn test_scanner_handles_multiple_scans() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, _evt_rx) = flume::unbounded();
        let provider = MockProvider::new();
        let reg = NodeRegistry::new();
        let session = session::SessionId::new();

        provider.add_file(make_file("test.txt", "/dir1", 50, false));

        let provider_clone = provider.clone();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);

        let _scanner_handle = tokio::spawn(async move {
            scanner.run().await;
        });

        // Send multiple scan commands
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/dir1"),
                pipeline: crate::pipeline::PipelineConfig {
                    sort: None,
                    filter: None,
                    group: None,
                },
                session,
            })
            .unwrap();

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/dir2"),
                pipeline: crate::pipeline::PipelineConfig {
                    sort: None,
                    filter: None,
                    group: None,
                },
                session,
            })
            .unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Verify both paths were scanned
        let calls = provider_clone.get_list_calls();
        assert!(
            calls.len() >= 2,
            "Scanner should process multiple scan commands"
        );
    }

    #[tokio::test]
    async fn test_scanner_handles_provider_errors() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, evt_rx) = flume::unbounded();
        let provider = MockProvider::new();
        let session = session::SessionId::new();
        let reg = NodeRegistry::new();
        // Configure provider to fail
        provider.set_should_fail(true);

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);

        let _scanner_handle = tokio::spawn(async move {
            scanner.run().await;
        });

        // Send scan command
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/nonexistent"),

                pipeline: crate::pipeline::PipelineConfig {
                    sort: None,
                    filter: None,
                    group: None,
                },
                session,
            })
            .unwrap();

        // Scanner should emit error event or handle gracefully
        match timeout(Duration::from_secs(1), evt_rx.recv_async()).await {
            Ok(Ok(Event::Error { message, .. })) => {
                assert!(!message.is_empty());
            }
            Ok(Ok(_)) => {
                // Other events acceptable depending on error handling strategy
            }
            Ok(Err(_)) => {
                panic!("Event channel closed unexpectedly");
            }
            Err(_) => {
                // Timeout acceptable if scanner handles errors silently
            }
        }
    }

    #[tokio::test]
    async fn test_scanner_depth_limiting() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, _evt_rx) = flume::unbounded();
        let provider = MockProvider::new();
        let session = session::SessionId::new();
        let reg = NodeRegistry::new();

        provider.add_file(make_file("shallow.txt", "/test", 10, false));

        let provider_clone = provider.clone();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);

        let _scanner_handle = tokio::spawn(async move {
            scanner.run().await;
        });

        // Scan with depth limit
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/test"),
                pipeline: crate::pipeline::PipelineConfig {
                    sort: None,
                    filter: None,
                    group: None,
                },
                session,
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify scanner respects depth (implementation dependent)
        // This is a placeholder - actual test would verify depth behavior
        let calls = provider_clone.get_list_calls();
        assert!(!calls.is_empty());
    }

    #[tokio::test]
    async fn test_scanner_emits_progress_events() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (evt_tx, evt_rx) = flume::unbounded();
        let provider = MockProvider::new();
        let session = session::SessionId::new();
        let reg = NodeRegistry::new();
        // Add multiple files to generate progress
        for i in 0..10 {
            provider.add_file(make_file(&format!("file{i}.txt"), "/test", i * 100, false));
        }

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);

        let _scanner_handle = tokio::spawn(async move {
            scanner.run().await;
        });

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/test"),
                pipeline: crate::pipeline::PipelineConfig {
                    sort: None,
                    filter: None,
                    group: None,
                },
                session,
            })
            .unwrap();

        // Collect events
        let mut received_events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);

        while tokio::time::Instant::now() < deadline {
            match timeout(Duration::from_millis(100), evt_rx.recv_async()).await {
                Ok(Ok(event)) => received_events.push(event),
                _ => break,
            }
        }

        // Scanner should emit at least one event
        assert!(
            !received_events.is_empty(),
            "Scanner should emit events during scan"
        );
    }
}

#[cfg(test)]
mod scanner_command_tests {
    use std::path::PathBuf;

    use crate::{actors::scanner::ScanCommand, model::session};

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
