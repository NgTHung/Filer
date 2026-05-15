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
    delay_ms: Arc<Mutex<u64>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
            delay_ms: Arc::new(Mutex::new(0)),
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

    fn set_delay_ms(&self, delay_ms: u64) {
        *self.delay_ms.lock().unwrap() = delay_ms;
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
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
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
        Err(CoreError::not_found(PathBuf::from("test")))
    }
}

#[cfg(test)]
mod scanner_cache_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use crate::actors::Actor;
    use crate::api::events::Event;
    use crate::model::location::{Location, LocationRef};
    use crate::model::registry::NodeRegistry;
    use crate::model::request::RequestId;
    use crate::model::session::SessionId;
    use crate::modules::scan::scanner::{ScanCommand, Scanner};
    use crate::pipeline::PipelineConfig;
    use crate::services::dir_cache::DirCache;
    use flume::Receiver;

    const SCAN_TIMEOUT: Duration = Duration::from_millis(2000);

    fn default_pipeline() -> PipelineConfig {
        PipelineConfig {
            sort: None,
            filter: None,
            group: None,
        }
    }

    async fn wait_for_dir_loaded(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> crate::pipeline::GroupedNodes {
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::DirectoryLoaded {
                    session: s, groups, ..
                })) if s == session => return groups,
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed waiting for DirectoryLoaded"),
            }
        }
    }

    async fn wait_for_location_dir_loaded(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> crate::pipeline::GroupedEntries {
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::DirectoryEntriesLoaded {
                    session: s, groups, ..
                })) if s == session => return groups,
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed waiting for DirectoryEntriesLoaded"),
            }
        }
    }

    #[tokio::test]
    async fn test_scan_location_emits_directory_entries_loaded() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/location-scan", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        let location = Location::local("/tmp/location-scan");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                request,
            })
            .unwrap();

        let groups = wait_for_location_dir_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "a.txt");
        assert_eq!(
            groups.groups[0].nodes[0].location.descriptor(),
            Some(&crate::model::location::LocationDescriptor::local(
                "/tmp/location-scan/a.txt"
            ))
        );
    }

    #[tokio::test]
    async fn test_scanner_uses_cache_on_second_scan() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/dir", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = std::path::PathBuf::from("/tmp/dir");
        let s1 = SessionId::new();
        let s2 = SessionId::new();

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: s1,
                pipeline: default_pipeline(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: s2,
                pipeline: default_pipeline(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s2).await;

        let calls = provider.get_list_calls();
        assert_eq!(
            calls.len(),
            1,
            "provider.list() should only be called once (second scan hits cache)"
        );
    }

    #[tokio::test]
    async fn test_scanner_bypasses_cache_after_invalidation() {
        let provider = MockProvider::new();
        provider.add_file(make_file("b.txt", "/tmp/dir2", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner = Scanner::with_cache(
            cmd_rx,
            evt_tx,
            Arc::new(provider.clone()),
            registry,
            cache.clone(),
        );
        tokio::spawn(async move { scanner.run().await });

        let path = std::path::PathBuf::from("/tmp/dir2");
        let s1 = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: s1,
                pipeline: default_pipeline(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        // Invalidate the cache entry
        cache.lock().unwrap().invalidate(&path);

        let s2 = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: s2,
                pipeline: default_pipeline(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s2).await;

        let calls = provider.get_list_calls();
        assert_eq!(
            calls.len(),
            2,
            "provider.list() should be called twice after cache invalidation"
        );
    }

    #[tokio::test]
    async fn test_refresh_node_invalidates_cache_before_scan() {
        let provider = MockProvider::new();
        provider.add_file(make_file("before.txt", "/tmp/refresh", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner = Scanner::with_cache(
            cmd_rx,
            evt_tx,
            Arc::new(provider.clone()),
            registry.clone(),
            cache,
        );
        tokio::spawn(async move { scanner.run().await });

        let path = std::path::PathBuf::from("/tmp/refresh");
        let node = registry.register(path);
        let s1 = SessionId::new();
        let s2 = SessionId::new();

        cmd_tx
            .send(ScanCommand::ScanNode {
                node,
                session: s1,
                pipeline: default_pipeline(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        provider.add_file(make_file("after.txt", "/tmp/refresh", 20, false));

        cmd_tx
            .send(ScanCommand::RefreshNode {
                node,
                session: s2,
                pipeline: default_pipeline(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_dir_loaded(&evt_rx, s2).await;

        let calls = provider.get_list_calls();
        assert_eq!(
            calls.len(),
            2,
            "RefreshNode should bypass cached directory entries"
        );
        assert_eq!(
            groups.total_count, 2,
            "RefreshNode should emit the fresh provider listing"
        );
    }

    #[tokio::test]
    async fn test_stale_scan_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.txt", "/tmp/stale", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let path = PathBuf::from("/tmp/stale");

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: default_pipeline(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline: default_pipeline(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryLoaded {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale scan request should not emit DirectoryLoaded"
        );
    }
}

// Scanner actor integration tests moved to filer-core/tests/scanner_integration_test.rs

#[cfg(test)]
mod scanner_command_tests {
    use std::path::PathBuf;

    use crate::{model::session, modules::scan::scanner::ScanCommand};

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
            request: crate::model::request::RequestId::new(),
        };

        let cloned = cmd.clone();

        match (cmd, cloned) {
            (
                ScanCommand::Scan {
                    path: p1,
                    pipeline: pl1,
                    session: s1,
                    request: _,
                },
                ScanCommand::Scan {
                    path: p2,
                    pipeline: pl2,
                    session: s2,
                    request: _,
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
            path: PathBuf::from("/test/path"),
            pipeline: crate::pipeline::PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session,
            request: crate::model::request::RequestId::new(),
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
