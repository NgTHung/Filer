use crate::errors::CoreError;
use crate::model::directory::{
    DirectoryCursor, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider, ListingOptions, ProviderPaging};
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
    page_calls: Arc<Mutex<Vec<(PathBuf, DirectoryPageRequest)>>>,
    list_options: Arc<Mutex<Vec<ListingOptions>>>,
    should_fail: Arc<Mutex<bool>>,
    delay_ms: Arc<Mutex<u64>>,
    native_paging: bool,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            page_calls: Arc::new(Mutex::new(Vec::new())),
            list_options: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
            delay_ms: Arc::new(Mutex::new(0)),
            native_paging: true,
        }
    }

    fn fallback() -> Self {
        Self {
            native_paging: false,
            ..Self::new()
        }
    }

    fn add_file(&self, node: FileNode) {
        self.files.lock().unwrap().push(node);
    }

    fn insert_file(&self, index: usize, node: FileNode) {
        self.files.lock().unwrap().insert(index, node);
    }

    fn get_list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    fn get_page_calls(&self) -> Vec<(PathBuf, DirectoryPageRequest)> {
        self.page_calls.lock().unwrap().clone()
    }

    fn get_list_options(&self) -> Vec<ListingOptions> {
        self.list_options.lock().unwrap().clone()
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

    fn paging(&self) -> ProviderPaging {
        if self.native_paging {
            ProviderPaging::Native
        } else {
            ProviderPaging::Fallback
        }
    }

    async fn list(
        &self,
        path: &Path,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<FileNode>, CoreError> {
        self.list_with_options(path, ListingOptions::default(), cx)
            .await
    }

    async fn list_with_options(
        &self,
        path: &Path,
        options: ListingOptions,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<FileNode>, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        self.list_calls.lock().unwrap().push(path.to_path_buf());
        self.list_options.lock().unwrap().push(options);
        Ok(self.files.lock().unwrap().clone())
    }

    async fn list_page(
        &self,
        path: &Path,
        request: DirectoryPageRequest,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<DirectoryPageResult, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        self.page_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), request.clone()));
        let start = request
            .cursor
            .as_ref()
            .and_then(|cursor| cursor.0.parse::<usize>().ok())
            .unwrap_or(0);
        let files = self.files.lock().unwrap();
        let end = (start + request.limit).min(files.len());
        let entries = files[start..end].to_vec();
        let next_cursor = (end < files.len()).then(|| DirectoryCursor(end.to_string()));
        let state = if let Some(cursor) = next_cursor {
            DirectoryPageState::partial(entries.len(), None, cursor)
        } else {
            DirectoryPageState::complete(entries.len(), None)
        };
        Ok(DirectoryPageResult { entries, state })
    }

    async fn read(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        _path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<FileNode, CoreError> {
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
    use crate::model::progress::{ProgressKind, ProgressPhase, ProgressStatus};
    use crate::model::registry::NodeRegistry;
    use crate::model::request::RequestId;
    use crate::model::session::SessionId;
    use crate::modules::scan::scanner::{ScanCommand, Scanner};
    use crate::pipeline::sort::{SortField, SortOrder};
    use crate::pipeline::{FilterConfig, GroupBy, PipelineConfig};
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

    fn snapshot_load() -> crate::DirectoryLoadOptions {
        crate::DirectoryLoadOptions::unbounded(ListingOptions::fast())
    }

    async fn wait_for_dir_loaded(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> crate::pipeline::GroupedNodes {
        wait_for_dir_loaded_with_state(evt_rx, session).await.0
    }

    async fn wait_for_dir_loaded_with_state(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> (crate::pipeline::GroupedNodes, crate::DirectoryLoadState) {
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::DirectoryLoadedCompat {
                    session: s,
                    groups,
                    load,
                    ..
                })) if s == session => return (groups, load),
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed waiting for DirectoryLoadedCompat"),
            }
        }
    }

    async fn wait_for_location_dir_loaded(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> crate::pipeline::GroupedEntries {
        wait_for_location_dir_loaded_with_state(evt_rx, session)
            .await
            .0
    }

    async fn wait_for_location_dir_loaded_with_state(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> (crate::pipeline::GroupedEntries, crate::DirectoryLoadState) {
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::DirectoryLoaded {
                    session: s,
                    groups,
                    load,
                    ..
                })) if s == session => return (groups, load),
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed waiting for DirectoryLoaded"),
            }
        }
    }

    async fn wait_for_dir_page_loaded(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> (crate::pipeline::GroupedNodes, crate::DirectoryPageState) {
        let (groups, page, _) = wait_for_dir_page_loaded_with_request(evt_rx, session).await;
        (groups, page)
    }

    async fn wait_for_dir_page_loaded_with_request(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> (
        crate::pipeline::GroupedNodes,
        crate::DirectoryPageState,
        RequestId,
    ) {
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::DirectoryPageLoadedCompat {
                    session: s,
                    groups,
                    page,
                    request,
                    ..
                })) if s == session => return (groups, page, request),
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed waiting for DirectoryPageLoadedCompat"),
            }
        }
    }

    async fn wait_for_location_dir_page_loaded(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> (crate::pipeline::GroupedEntries, crate::DirectoryPageState) {
        let (groups, page, _) =
            wait_for_location_dir_page_loaded_with_request(evt_rx, session).await;
        (groups, page)
    }

    async fn wait_for_location_dir_page_loaded_with_request(
        evt_rx: &Receiver<Event>,
        session: SessionId,
    ) -> (
        crate::pipeline::GroupedEntries,
        crate::DirectoryPageState,
        RequestId,
    ) {
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::DirectoryPageLoaded {
                    session: s,
                    groups,
                    page,
                    request,
                    ..
                })) if s == session => return (groups, page, request),
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed waiting for DirectoryPageLoaded"),
            }
        }
    }

    async fn collect_until_dir_loaded(evt_rx: &Receiver<Event>, session: SessionId) -> Vec<Event> {
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(event @ Event::DirectoryLoadedCompat { session: s, .. })) if s == session => {
                    events.push(event);
                    events.extend(collect_for_duration(evt_rx, Duration::from_millis(50)).await);
                    return events;
                }
                Ok(Ok(event)) => events.push(event),
                _ => panic!("timed out or channel closed waiting for DirectoryLoadedCompat"),
            }
        }
    }

    async fn collect_for_duration(evt_rx: &Receiver<Event>, duration: Duration) -> Vec<Event> {
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + duration;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            events.push(event);
        }
        events
    }

    #[tokio::test]
    async fn test_scan_emits_generic_progress_through_completion() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/progress", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/progress"),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request,
            })
            .unwrap();

        let events = collect_until_dir_loaded(&evt_rx, session).await;
        let progress: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::ProgressUpdated { scope, snapshot }
                    if scope.session == session
                        && scope.request == Some(request)
                        && scope.kind == ProgressKind::Scan =>
                {
                    Some(snapshot)
                }
                _ => None,
            })
            .collect();

        assert!(progress.iter().any(|p| p.status == ProgressStatus::Started));
        assert!(
            progress
                .iter()
                .any(|p| p.phase == ProgressPhase::CacheLookup)
        );
        assert!(progress.iter().any(|p| p.phase == ProgressPhase::Emitting));
        assert!(
            progress
                .iter()
                .any(|p| p.status == ProgressStatus::Completed)
        );
    }

    #[tokio::test]
    async fn test_scan_cancel_emits_cancelled_progress() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("a.txt", "/tmp/progress-cancel", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/progress-cancel"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request,
            })
            .unwrap();
        cmd_tx.send(ScanCommand::Cancel(session)).unwrap();

        let events = collect_for_duration(&evt_rx, Duration::from_millis(200)).await;
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated {
                    scope,
                    snapshot
                } if scope.session == session
                    && scope.request == Some(request)
                    && snapshot.status == ProgressStatus::Cancelled
            )
        }));
        assert!(!events.iter().any(|event| {
            matches!(event, Event::DirectoryLoadedCompat { session: s, .. } if *s == session)
        }));
        assert!(!events.iter().any(|event| {
            matches!(event, Event::DirectoryPageLoadedCompat { session: s, .. } if *s == session)
        }));
        assert!(!events.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated {
                    scope,
                    snapshot
                } if scope.session == session
                    && scope.request == Some(request)
                    && snapshot.status == ProgressStatus::Completed
            )
        }));
    }

    #[tokio::test]
    async fn test_scan_location_page_cancel_suppresses_directory_entry_page_loaded() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("a.txt", "/tmp/location-page-cancel", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        let location = Location::local("/tmp/location-page-cancel");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request,
            })
            .unwrap();
        cmd_tx.send(ScanCommand::Cancel(session)).unwrap();

        let events = collect_for_duration(&evt_rx, Duration::from_millis(200)).await;
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated {
                    scope,
                    snapshot
                } if scope.session == session
                    && scope.request == Some(request)
                    && snapshot.status == ProgressStatus::Cancelled
            )
        }));
        assert!(!events.iter().any(|event| {
            matches!(event, Event::DirectoryPageLoaded { session: s, .. } if *s == session)
        }));
        assert!(!events.iter().any(|event| {
            matches!(event, Event::DirectoryLoaded { session: s, .. } if *s == session)
        }));
        assert!(!events.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated {
                    scope,
                    snapshot
                } if scope.session == session
                    && scope.request == Some(request)
                    && snapshot.status == ProgressStatus::Completed
            )
        }));
    }

    #[tokio::test]
    async fn test_scan_failure_emits_failed_progress_before_error() {
        let provider = MockProvider::new();
        provider.set_should_fail(true);

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/progress-fail"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request,
            })
            .unwrap();

        let events = collect_for_duration(&evt_rx, Duration::from_millis(200)).await;
        let failed_index = events.iter().position(|event| {
            matches!(
                event,
                Event::ProgressUpdated {
                    scope,
                    snapshot
                } if scope.session == session
                    && scope.request == Some(request)
                    && snapshot.status == ProgressStatus::Failed
            )
        });
        let error_index = events.iter().position(|event| {
            matches!(
                event,
                Event::Error {
                    session: s,
                    request: Some(r),
                    ..
                } if *s == session && *r == request
            )
        });

        assert!(failed_index.is_some());
        assert!(error_index.is_some());
        assert!(failed_index < error_index);
    }

    #[tokio::test]
    async fn test_default_scan_emits_directory_page_loaded() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/page", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/page", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/page"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(provider.get_page_calls().len(), 1);
        assert_eq!(groups.total_count, 2);
        assert_eq!(page.page_count, 2);
        assert!(page.complete);
    }

    #[tokio::test]
    async fn test_scan_page_after_uses_cursor_and_emits_next_page() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/page-cursor", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/page-cursor", 20, false));
        provider.add_file(make_file("c.txt", "/tmp/page-cursor", 30, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/page-cursor"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();
        let (_, first_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        let next_cursor = first_page.next_cursor.expect("first page should continue");

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/page-cursor"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page_after(2, next_cursor),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, second_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(second_page.next_cursor, None);
        assert!(second_page.complete);
        assert_eq!(provider.get_page_calls().len(), 2);
    }

    #[tokio::test]
    async fn test_scan_location_default_emits_directory_entry_page_loaded() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/location-page", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let location = Location::local("/tmp/location-page");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_location_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert!(page.complete);
    }

    #[tokio::test]
    async fn test_repeated_page_requests_same_session_use_next_cursor_and_request_ids() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/repeated-page", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/repeated-page", 20, false));
        provider.add_file(make_file("c.txt", "/tmp/repeated-page", 30, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let first_request = RequestId::new();
        let second_request = RequestId::new();
        let path = PathBuf::from("/tmp/repeated-page");

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(2),
                request: first_request,
            })
            .unwrap();
        let (_, first_page, emitted_first_request) =
            wait_for_dir_page_loaded_with_request(&evt_rx, session).await;
        let next_cursor = first_page
            .next_cursor
            .clone()
            .expect("first page should have a continuation cursor");
        assert_eq!(emitted_first_request, first_request);

        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page_after(2, next_cursor.clone()),
                request: second_request,
            })
            .unwrap();
        let (groups, second_page, emitted_second_request) =
            wait_for_dir_page_loaded_with_request(&evt_rx, session).await;

        assert_eq!(emitted_second_request, second_request);
        assert_eq!(groups.total_count, 1);
        assert!(second_page.complete);
        assert_eq!(second_page.next_cursor, None);

        let page_calls = provider.get_page_calls();
        assert_eq!(page_calls.len(), 2);
        assert_eq!(page_calls[0].1.cursor, None);
        assert_eq!(page_calls[1].1.cursor, None);
    }

    #[tokio::test]
    async fn test_partial_page_does_not_populate_complete_cache() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/partial-page-cache", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/partial-page-cache", 20, false));
        provider.add_file(make_file("c.txt", "/tmp/partial-page-cache", 30, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/partial-page-cache");
        for _ in 0..2 {
            let session = SessionId::new();
            cmd_tx
                .send(ScanCommand::Scan {
                    path: path.clone(),
                    session,
                    pipeline: default_pipeline(),
                    load: crate::DirectoryLoadOptions::page(2),
                    request: RequestId::new(),
                })
                .unwrap();
            let (_, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
            assert!(!page.complete);
        }

        assert_eq!(
            provider.get_page_calls().len(),
            2,
            "partial pages must not populate complete directory cache entries"
        );
    }

    #[tokio::test]
    async fn test_complete_page_can_be_served_from_cache() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/complete-page-cache", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/complete-page-cache", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/complete-page-cache");
        for _ in 0..2 {
            let session = SessionId::new();
            cmd_tx
                .send(ScanCommand::Scan {
                    path: path.clone(),
                    session,
                    pipeline: default_pipeline(),
                    load: crate::DirectoryLoadOptions::page(10),
                    request: RequestId::new(),
                })
                .unwrap();
            let (groups, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
            assert_eq!(groups.total_count, 2);
            assert!(page.complete);
        }

        assert_eq!(
            provider.get_page_calls().len(),
            1,
            "complete first pages may be cached and reused for page requests"
        );
    }

    #[tokio::test]
    async fn test_refresh_node_page_invalidates_cached_complete_page() {
        let provider = MockProvider::new();
        provider.add_file(make_file("before.txt", "/tmp/page-refresh", 10, false));

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

        let path = PathBuf::from("/tmp/page-refresh");
        let node = registry.register(path.clone());

        let scan_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session: scan_session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(10),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_page_loaded(&evt_rx, scan_session).await;

        provider.add_file(make_file("after.txt", "/tmp/page-refresh", 20, false));

        let refresh_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::RefreshNode {
                node,
                session: refresh_session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(10),
                request: RequestId::new(),
            })
            .unwrap();
        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, refresh_session).await;

        assert_eq!(provider.get_page_calls().len(), 2);
        assert_eq!(groups.total_count, 2);
        assert_eq!(page.page_count, 2);
        assert!(page.complete);
    }

    #[tokio::test]
    async fn test_refresh_starts_new_cursor_generation_after_mutation() {
        let provider = MockProvider::new();
        provider.add_file(make_file(
            "a.txt",
            "/tmp/page-refresh-generation",
            10,
            false,
        ));
        provider.add_file(make_file(
            "b.txt",
            "/tmp/page-refresh-generation",
            20,
            false,
        ));

        let registry = NodeRegistry::new();
        let node = registry
            .clone()
            .register(PathBuf::from("/tmp/page-refresh-generation"));
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanNode {
                node,
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();
        let (_, first_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        let stale_cursor = first_page.next_cursor.expect("first page should continue");

        provider.insert_file(
            0,
            make_file("0.txt", "/tmp/page-refresh-generation", 5, false),
        );
        cmd_tx
            .send(ScanCommand::RefreshNode {
                node,
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();
        let (groups, refreshed) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.groups[0].nodes[0].name, "0.txt");
        assert_eq!(refreshed.start_index, 0);

        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::ScanNode {
                node,
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page_after(1, stale_cursor),
                request,
            })
            .unwrap();
        let events = collect_for_duration(&evt_rx, Duration::from_millis(200)).await;
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::Error {
                    request: Some(error_request),
                    ..
                } if *error_request == request
            )
        }));
    }

    #[tokio::test]
    async fn test_page_request_with_sort_uses_incremental_page_event() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/page-fallback", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/page-fallback"),
                session,
                pipeline: PipelineConfig::with_default_sort(),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert!(page.complete);
        assert!(!provider.get_page_calls().is_empty());
        assert_eq!(provider.get_list_calls().len(), 0);
    }

    #[tokio::test]
    async fn test_sorted_pages_continue_in_pipeline_order() {
        let provider = MockProvider::new();
        provider.add_file(make_file("c.txt", "/tmp/sorted-page", 30, false));
        provider.add_file(make_file("a.txt", "/tmp/sorted-page", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/sorted-page", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let pipeline = PipelineConfig::default().sort(SortField::Name, SortOrder::Ascending, true);
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/sorted-page"),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();
        let (first, state) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(
            first.groups[0]
                .nodes
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.txt", "b.txt"]
        );
        let cursor = state.next_cursor.expect("sorted page should continue");

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/sorted-page"),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::page_after(2, cursor),
                request: RequestId::new(),
            })
            .unwrap();
        let (second, state) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(second.groups[0].nodes[0].name, "c.txt");
        assert_eq!(state.start_index, 2);
        assert_eq!(state.loaded_count, 3);
        assert!(state.complete);
    }

    #[tokio::test]
    async fn test_grouped_pages_continue_by_group_then_name() {
        let provider = MockProvider::new();
        provider.add_file(make_file("z.rs", "/tmp/grouped-page", 30, false));
        provider.add_file(make_file("b.txt", "/tmp/grouped-page", 20, false));
        provider.add_file(make_file("a.rs", "/tmp/grouped-page", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let pipeline = PipelineConfig::default().group_by(GroupBy::Extension);
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/grouped-page"),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();
        let (first, state) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(first.groups[0].label, "rs");
        assert_eq!(
            first.groups[0]
                .nodes
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.rs", "z.rs"]
        );
        let cursor = state.next_cursor.expect("grouped page should continue");

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/grouped-page"),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::page_after(2, cursor),
                request: RequestId::new(),
            })
            .unwrap();
        let (second, state) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(second.groups[0].label, "txt");
        assert_eq!(second.groups[0].nodes[0].name, "b.txt");
        assert!(state.complete);
    }

    #[tokio::test]
    async fn test_fallback_provider_materializes_once_per_page_request() {
        let provider = MockProvider::fallback();
        provider.add_file(make_file("c.txt", "/tmp/fallback-page", 30, false));
        provider.add_file(make_file("a.txt", "/tmp/fallback-page", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/fallback-page", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/fallback-page"),
                session,
                pipeline: PipelineConfig::default().sort(
                    SortField::Size,
                    SortOrder::Ascending,
                    true,
                ),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();
        let (_, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert!(!page.complete);
        assert_eq!(provider.get_list_calls().len(), 1);
        assert_eq!(
            provider.get_list_options(),
            vec![ListingOptions::metadata()]
        );
        assert!(provider.get_page_calls().is_empty());
    }

    #[tokio::test]
    async fn test_filter_only_page_uses_provider_pages() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/filter-page", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/filter-page", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/filter-page", 30, false));
        provider.add_file(make_file("d.txt", "/tmp/filter-page", 40, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-page"),
                session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::only_extensions(vec!["rs".into()])),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 2);
        assert_eq!(groups.groups[0].nodes[0].name, "a.rs");
        assert_eq!(groups.groups[0].nodes[1].name, "c.rs");
        assert!(page.complete);
        assert!(page.next_cursor.is_none());
        assert!(!provider.get_page_calls().is_empty());
        assert_eq!(provider.get_list_calls().len(), 0);
    }

    #[tokio::test]
    async fn test_filter_page_cursor_continues_filtered_results() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/filter-cursor", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/filter-cursor", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/filter-cursor", 30, false));
        provider.add_file(make_file("d.rs", "/tmp/filter-cursor", 40, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-cursor"),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();

        let (first_groups, first_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(
            first_groups.groups[0]
                .nodes
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.rs", "c.rs"]
        );
        let next_cursor = first_page.next_cursor.expect("first page should continue");

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-cursor"),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::page_after(2, next_cursor),
                request: RequestId::new(),
            })
            .unwrap();

        let (second_groups, second_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(second_groups.total_count, 1);
        assert_eq!(second_groups.groups[0].nodes[0].name, "d.rs");
        assert!(second_page.complete);
        assert_eq!(provider.get_page_calls().len(), 2);
    }

    #[tokio::test]
    async fn test_sparse_filter_page_finds_late_match_with_bounded_memory() {
        let provider = MockProvider::new();
        for idx in 0..300 {
            provider.add_file(make_file(
                &format!("skip-{idx}.txt"),
                "/tmp/filter-budget",
                idx,
                false,
            ));
        }
        provider.add_file(make_file("late.rs", "/tmp/filter-budget", 500, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-budget"),
                session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::only_extensions(vec!["rs".into()])),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "late.rs");
        assert_eq!(page.page_count, 1);
        assert!(page.complete);
        assert!(page.next_cursor.is_none());
        assert_eq!(provider.get_list_calls().len(), 0);
        assert_eq!(provider.get_page_calls().len(), 2);
    }

    #[tokio::test]
    async fn test_sparse_filter_returns_complete_page_without_empty_intermediate_page() {
        let provider = MockProvider::new();
        for idx in 0..300 {
            provider.add_file(make_file(
                &format!("skip-{idx}.txt"),
                "/tmp/filter-empty-continue",
                idx,
                false,
            ));
        }
        provider.add_file(make_file(
            "late.rs",
            "/tmp/filter-empty-continue",
            500,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-empty-continue"),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "late.rs");
        assert!(page.complete);
    }

    #[tokio::test]
    async fn test_cancel_filtered_page_suppresses_directory_page_loaded() {
        let provider = MockProvider::new();
        provider.set_delay_ms(10);
        for idx in 0..1000 {
            provider.add_file(make_file(
                &format!("skip-{idx}.txt"),
                "/tmp/filter-cancel",
                idx,
                false,
            ));
        }

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-cancel"),
                session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::only_extensions(vec!["rs".into()])),
                load: crate::DirectoryLoadOptions::page(1),
                request,
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await;
        cmd_tx.send(ScanCommand::Cancel(session)).unwrap();

        let events = collect_for_duration(&evt_rx, Duration::from_millis(250)).await;
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated { scope, snapshot }
                    if scope.session == session
                        && scope.request == Some(request)
                        && snapshot.status == ProgressStatus::Cancelled
            )
        }));
        assert!(!events.iter().any(|event| {
            matches!(event, Event::DirectoryPageLoadedCompat { session: s, .. } if *s == session)
        }));
        assert!(!events.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated { scope, snapshot }
                    if scope.session == session
                        && scope.request == Some(request)
                        && snapshot.status == ProgressStatus::Completed
            )
        }));
        assert!(
            provider.get_page_calls().len() < 4,
            "cancellation should stop the sparse filtered loop before the raw budget is exhausted"
        );
    }

    #[tokio::test]
    async fn test_filtered_cursor_does_not_duplicate_unchanged_rows_under_mutation() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/filter-mutation", 10, false));
        provider.add_file(make_file("b.rs", "/tmp/filter-mutation", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/filter-mutation", 30, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-mutation"),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();
        let (first_groups, first_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(first_groups.groups[0].nodes[0].name, "a.rs");
        let cursor = first_page.next_cursor.expect("first page should continue");

        provider.insert_file(0, make_file("new.rs", "/tmp/filter-mutation", 5, false));

        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-mutation"),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::page_after(1, cursor),
                request: RequestId::new(),
            })
            .unwrap();
        let (second_groups, second_page) = wait_for_dir_page_loaded(&evt_rx, session).await;

        assert_eq!(
            second_groups.groups[0].nodes[0].name, "b.rs",
            "unchanged rows must not repeat when an insertion precedes the cursor"
        );
        assert_eq!(second_page.start_index, 1);
        assert_eq!(second_page.loaded_count, 2);
    }

    #[tokio::test]
    async fn test_filtered_cursor_rejects_changed_pipeline() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/filter-mismatch", 10, false));
        provider.add_file(make_file("b.rs", "/tmp/filter-mismatch", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/filter-mismatch", 30, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let first_pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
        let first_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-mismatch"),
                session: first_session,
                pipeline: first_pipeline,
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();
        let (_, first_page) = wait_for_dir_page_loaded(&evt_rx, first_session).await;
        let cursor = first_page.next_cursor.expect("first page should continue");

        let second_session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-mismatch"),
                session: second_session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::exclude_extensions(vec!["tmp".into()])),
                load: crate::DirectoryLoadOptions::page_after(1, cursor),
                request,
            })
            .unwrap();

        let events = collect_for_duration(&evt_rx, Duration::from_millis(200)).await;
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::Error {
                    session,
                    request: Some(error_request),
                    ..
                } if *session == second_session && *error_request == request
            )
        }));
    }

    #[tokio::test]
    async fn test_complete_cache_can_serve_filter_page_without_provider_page_call() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/filter-cache", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/filter-cache", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/filter-cache", 30, false));

        let registry = NodeRegistry::new();
        let cache = Arc::new(Mutex::new(DirCache::new(1024 * 1024)));
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let snapshot_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-cache"),
                session: snapshot_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, snapshot_session).await;

        let filter_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: PathBuf::from("/tmp/filter-cache"),
                session: filter_session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::only_extensions(vec!["rs".into()])),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_dir_page_loaded(&evt_rx, filter_session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "a.rs");
        assert!(!page.complete);
        assert_eq!(provider.get_list_calls().len(), 1);
        assert_eq!(provider.get_page_calls().len(), 0);
    }

    #[tokio::test]
    async fn test_scan_location_filter_page_emits_directory_entry_page_loaded() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/location-filter-page", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/location-filter-page", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/location-filter-page", 30, false));
        provider.add_file(make_file("d.txt", "/tmp/location-filter-page", 40, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let location = Location::local("/tmp/location-filter-page");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::only_extensions(vec!["rs".into()])),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, page) = wait_for_location_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 2);
        assert_eq!(groups.groups[0].nodes[0].name, "a.rs");
        assert_eq!(groups.groups[0].nodes[1].name, "c.rs");
        assert!(page.complete);
        assert!(page.next_cursor.is_none());
        assert_eq!(provider.get_list_calls().len(), 0);
        assert!(!provider.get_page_calls().is_empty());
    }

    #[tokio::test]
    async fn test_scan_location_filter_page_cursor_continues() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.rs", "/tmp/location-filter-cursor", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/location-filter-cursor", 20, false));
        provider.add_file(make_file("c.rs", "/tmp/location-filter-cursor", 30, false));
        provider.add_file(make_file("d.rs", "/tmp/location-filter-cursor", 40, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
        let location = Location::local("/tmp/location-filter-cursor");
        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();

        let (first_groups, first_page) = wait_for_location_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(
            first_groups.groups[0]
                .nodes
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.rs", "c.rs"]
        );
        let cursor = first_page.next_cursor.expect("first page should continue");

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::page_after(2, cursor),
                request: RequestId::new(),
            })
            .unwrap();

        let (second_groups, second_page) =
            wait_for_location_dir_page_loaded(&evt_rx, session).await;
        assert_eq!(second_groups.total_count, 1);
        assert_eq!(second_groups.groups[0].nodes[0].name, "d.rs");
        assert!(second_page.complete);
    }

    #[tokio::test]
    async fn test_scan_location_filter_cursor_rejects_changed_pipeline() {
        let provider = MockProvider::new();
        provider.add_file(make_file(
            "a.rs",
            "/tmp/location-filter-mismatch",
            10,
            false,
        ));
        provider.add_file(make_file(
            "b.rs",
            "/tmp/location-filter-mismatch",
            20,
            false,
        ));
        provider.add_file(make_file(
            "c.rs",
            "/tmp/location-filter-mismatch",
            30,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let location = Location::local("/tmp/location-filter-mismatch");
        let first_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: first_session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::only_extensions(vec!["rs".into()])),
                load: crate::DirectoryLoadOptions::page(1),
                request: RequestId::new(),
            })
            .unwrap();
        let (_, first_page) = wait_for_location_dir_page_loaded(&evt_rx, first_session).await;
        let cursor = first_page.next_cursor.expect("first page should continue");

        let second_session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: second_session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig::exclude_extensions(vec!["tmp".into()])),
                load: crate::DirectoryLoadOptions::page_after(1, cursor),
                request,
            })
            .unwrap();

        let events = collect_for_duration(&evt_rx, Duration::from_millis(200)).await;
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::Error {
                    session,
                    request: Some(error_request),
                    ..
                } if *session == second_session && *error_request == request
            )
        }));
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
                load: snapshot_load(),
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
                load: snapshot_load(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: s2,
                pipeline: default_pipeline(),
                load: snapshot_load(),
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
    async fn test_scanner_forwards_listing_options_to_provider() {
        let provider = MockProvider::new();
        provider.add_file(make_file("metadata.txt", "/tmp/dir", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider.clone()), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: std::path::PathBuf::from("/tmp/dir"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::unbounded(ListingOptions::metadata()),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, session).await;

        assert_eq!(
            provider.get_list_options(),
            vec![ListingOptions::metadata()]
        );
    }

    #[tokio::test]
    async fn test_scanner_cache_separates_listing_options() {
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
        let fast_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: fast_session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::unbounded(ListingOptions::fast()),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, fast_session).await;

        let metadata_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session: metadata_session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::unbounded(ListingOptions::metadata()),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, metadata_session).await;

        assert_eq!(
            provider.get_list_options(),
            vec![ListingOptions::fast(), ListingOptions::metadata()]
        );
    }

    #[tokio::test]
    async fn test_bounded_scan_emits_limited_directory_loaded_state() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/bounded", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/bounded", 20, false));
        provider.add_file(make_file("c.txt", "/tmp/bounded", 30, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: std::path::PathBuf::from("/tmp/bounded"),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::bounded(2),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, load) = wait_for_dir_loaded_with_state(&evt_rx, session).await;
        assert_eq!(groups.total_count, 2);
        assert_eq!(groups.groups[0].nodes.len(), 2);
        assert_eq!(load.loaded_count, 2);
        assert_eq!(load.total_count, Some(3));
        assert!(!load.complete);
    }

    #[tokio::test]
    async fn test_bounded_scan_location_emits_limited_directory_entries_loaded_state() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/location-bounded", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/location-bounded", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let location = Location::local("/tmp/location-bounded");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::bounded(1),
                request: RequestId::new(),
            })
            .unwrap();

        let (groups, load) = wait_for_location_dir_loaded_with_state(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes.len(), 1);
        assert_eq!(load.loaded_count, 1);
        assert_eq!(load.total_count, Some(2));
        assert!(!load.complete);
    }

    #[tokio::test]
    async fn test_bounded_scan_does_not_populate_complete_cache() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/no-cache", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/no-cache", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = std::path::PathBuf::from("/tmp/no-cache");
        for _ in 0..2 {
            let session = SessionId::new();
            cmd_tx
                .send(ScanCommand::Scan {
                    path: path.clone(),
                    session,
                    pipeline: default_pipeline(),
                    load: crate::DirectoryLoadOptions::bounded(1),
                    request: RequestId::new(),
                })
                .unwrap();
            wait_for_dir_loaded(&evt_rx, session).await;
        }

        assert_eq!(
            provider.get_list_calls().len(),
            2,
            "bounded scans must not populate complete directory cache entries"
        );
    }

    #[tokio::test]
    async fn test_bounded_scan_can_reuse_complete_cached_listing() {
        let provider = MockProvider::new();
        provider.add_file(make_file("a.txt", "/tmp/reuse-cache", 10, false));
        provider.add_file(make_file("b.txt", "/tmp/reuse-cache", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = std::path::PathBuf::from("/tmp/reuse-cache");
        let full_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: full_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, full_session).await;

        let bounded_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session: bounded_session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::bounded(1),
                request: RequestId::new(),
            })
            .unwrap();
        let (groups, load) = wait_for_dir_loaded_with_state(&evt_rx, bounded_session).await;

        assert_eq!(provider.get_list_calls().len(), 1);
        assert_eq!(groups.total_count, 1);
        assert_eq!(load.total_count, Some(2));
        assert!(!load.complete);
    }

    #[tokio::test]
    async fn test_scan_location_cache_hit_emits_directory_entries_loaded() {
        let provider = MockProvider::new();
        provider.add_file(make_file("cached.txt", "/tmp/location-cache", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let location = Location::local("/tmp/location-cache");
        let s1 = SessionId::new();
        let s2 = SessionId::new();

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: s1,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_location_dir_loaded(&evt_rx, s1).await;

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: s2,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_location_dir_loaded(&evt_rx, s2).await;

        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "cached.txt");
        assert_eq!(
            provider.get_list_calls().len(),
            1,
            "second ScanLocation should hit cache but still emit DirectoryLoaded"
        );
    }

    #[tokio::test]
    async fn test_legacy_scan_reuses_cache_populated_by_scan_location() {
        let provider = MockProvider::new();
        provider.add_file(make_file(
            "cached.txt",
            "/tmp/location-to-path-cache",
            10,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/location-to-path-cache");
        let location = Location::local(path.clone());

        let location_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: location_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_location_dir_loaded(&evt_rx, location_session).await;

        let legacy_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session: legacy_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_dir_loaded(&evt_rx, legacy_session).await;

        assert_eq!(groups.total_count, 1);
        assert_eq!(provider.get_list_calls().len(), 1);
    }

    #[tokio::test]
    async fn test_scan_location_reuses_cache_populated_by_legacy_scan() {
        let provider = MockProvider::new();
        provider.add_file(make_file(
            "cached.txt",
            "/tmp/path-to-location-cache",
            10,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/path-to-location-cache");
        let legacy_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: legacy_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, legacy_session).await;

        let location_session = SessionId::new();
        let location = Location::local(path);
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: location_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_location_dir_loaded(&evt_rx, location_session).await;

        assert_eq!(groups.total_count, 1);
        assert_eq!(provider.get_list_calls().len(), 1);
    }

    #[tokio::test]
    async fn test_refresh_location_invalidates_location_and_path_cache() {
        let provider = MockProvider::new();
        provider.add_file(make_file(
            "before.txt",
            "/tmp/location-refresh-cache",
            10,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/location-refresh-cache");
        let location = Location::local(path.clone());

        let scan_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: scan_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_location_dir_loaded(&evt_rx, scan_session).await;

        provider.add_file(make_file(
            "after.txt",
            "/tmp/location-refresh-cache",
            20,
            false,
        ));

        let refresh_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::RefreshLocation {
                location: LocationRef::from_location(&location),
                session: refresh_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_location_dir_loaded(&evt_rx, refresh_session).await;

        assert_eq!(provider.get_list_calls().len(), 2);
        assert_eq!(groups.total_count, 2);

        let legacy_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session: legacy_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let legacy_groups = wait_for_dir_loaded(&evt_rx, legacy_session).await;

        assert_eq!(provider.get_list_calls().len(), 2);
        assert_eq!(legacy_groups.total_count, 2);
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
                load: snapshot_load(),
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
                load: snapshot_load(),
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
    async fn test_refresh_node_bypasses_cache_after_location_scan() {
        let provider = MockProvider::new();
        provider.add_file(make_file("before.txt", "/tmp/location-refresh", 10, false));

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

        let path = PathBuf::from("/tmp/location-refresh");
        let node = registry.clone().register(path.clone());
        let location = Location::local(path);
        let s1 = SessionId::new();
        let s2 = SessionId::new();

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session: s1,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_location_dir_loaded(&evt_rx, s1).await;

        provider.add_file(make_file("after.txt", "/tmp/location-refresh", 20, false));

        cmd_tx
            .send(ScanCommand::RefreshNode {
                node,
                session: s2,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_dir_loaded(&evt_rx, s2).await;

        assert_eq!(provider.get_list_calls().len(), 2);
        assert_eq!(
            groups.total_count, 2,
            "RefreshNode should bypass cache populated by ScanLocation"
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
                load: snapshot_load(),
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
                load: snapshot_load(),
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
    async fn test_stale_scan_location_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.txt", "/tmp/location-stale", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/tmp/location-stale");

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
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
            "stale ScanLocation request should not emit DirectoryLoaded"
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
                load: snapshot_load(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryLoadedCompat {
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
            "stale scan request should not emit DirectoryLoadedCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_scan_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.txt", "/tmp/stale-page", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let path = PathBuf::from("/tmp/stale-page");

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoadedCompat {
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
            "stale scan request should not emit DirectoryPageLoadedCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_filtered_scan_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.rs", "/tmp/filter-stale-page", 10, false));
        provider.add_file(make_file("skip.txt", "/tmp/filter-stale-page", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let path = PathBuf::from("/tmp/filter-stale-page");
        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoadedCompat {
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
            "stale filtered scan request should not emit DirectoryPageLoadedCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_scan_location_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file(
            "fresh.txt",
            "/tmp/location-stale-page",
            10,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/tmp/location-stale-page");

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoaded {
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
            "stale ScanLocation request should not emit DirectoryPageLoaded"
        );
    }

    #[tokio::test]
    async fn test_stale_filtered_scan_location_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file(
            "fresh.rs",
            "/tmp/location-filter-stale-page",
            10,
            false,
        ));
        provider.add_file(make_file(
            "skip.txt",
            "/tmp/location-filter-stale-page",
            20,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/tmp/location-filter-stale-page");
        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoaded {
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
            "stale filtered ScanLocation request should not emit DirectoryPageLoaded"
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
            load: crate::DirectoryLoadOptions::default(),
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
                    ..
                },
                ScanCommand::Scan {
                    path: p2,
                    pipeline: pl2,
                    session: s2,
                    request: _,
                    ..
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
            load: crate::DirectoryLoadOptions::default(),
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

        let result = provider
            .list(Path::new("/test"), &crate::ProviderCx::none())
            .await;

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "test.txt");
    }

    #[tokio::test]
    async fn test_mock_provider_tracks_calls() {
        let provider = MockProvider::new();

        provider
            .list(Path::new("/dir1"), &crate::ProviderCx::none())
            .await
            .unwrap();
        provider
            .list(Path::new("/dir2"), &crate::ProviderCx::none())
            .await
            .unwrap();

        let calls = provider.get_list_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], PathBuf::from("/dir1"));
        assert_eq!(calls[1], PathBuf::from("/dir2"));
    }

    #[tokio::test]
    async fn test_mock_provider_can_fail() {
        let provider = MockProvider::new();
        provider.set_should_fail(true);

        let result = provider
            .list(Path::new("/test"), &crate::ProviderCx::none())
            .await;
        assert!(result.is_err());
    }
}
