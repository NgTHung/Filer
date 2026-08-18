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
    use std::io::Write;
    use zip::write::SimpleFileOptions;

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
    ) -> crate::pipeline::GroupedEntries {
        wait_for_dir_loaded_with_state(evt_rx, session).await.0
    }

    async fn wait_for_dir_loaded_with_state(
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
    ) -> (crate::pipeline::GroupedEntries, crate::DirectoryPageState) {
        let (groups, page, _) = wait_for_dir_page_loaded_with_request(evt_rx, session).await;
        (groups, page)
    }

    async fn wait_for_dir_page_loaded_with_request(
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
                Ok(Ok(event @ Event::DirectoryLoaded { session: s, .. })) if s == session => {
                    events.push(event);
                    events.extend(collect_for_duration(evt_rx, Duration::from_millis(50)).await);
                    return events;
                }
                Ok(Ok(event)) => events.push(event),
                _ => panic!("timed out or channel closed waiting for DirectoryLoaded"),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/progress")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/progress-cancel")),
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
            matches!(event, Event::DirectoryLoaded { session: s, .. } if *s == session)
        }));
        assert!(!events.iter().any(|event| {
            matches!(event, Event::DirectoryPageLoaded { session: s, .. } if *s == session)
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
    async fn test_scan_location_segmented_zip_emits_directory_entries_loaded() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("bundle.zip");
        write_zip(
            &archive,
            &[("src/lib.rs", b"pub fn lib() {}"), ("README.md", b"readme")],
        );

        let registry = NodeRegistry::new();
        let provider = crate::LocalFs::new(registry.clone());
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        let descriptor = crate::LocationDescriptor::local(&archive).archive_member("");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::descriptor_only(descriptor.clone()),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request,
            })
            .unwrap();

        let groups = wait_for_location_dir_loaded(&evt_rx, session).await;

        assert_eq!(groups.total_count, 2);
        let nodes = &groups.groups[0].nodes;
        let src = nodes.iter().find(|node| node.name == "src").unwrap();
        assert!(src.capabilities.navigate);
        assert_eq!(
            src.location.descriptor(),
            Some(&crate::LocationDescriptor::local(&archive).archive_member("src"))
        );
        let readme = nodes.iter().find(|node| node.name == "README.md").unwrap();
        assert!(!readme.capabilities.navigate);
        assert_eq!(
            readme.location.descriptor(),
            Some(&crate::LocationDescriptor::local(&archive).archive_member("README.md"))
        );
    }

    #[tokio::test]
    async fn test_scan_location_segmented_zip_applies_pipeline_config() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("bundle.zip");
        write_zip(
            &archive,
            &[
                ("src/lib.rs", b"pub fn lib() {}"),
                ("main.rs", b"fn main() {}"),
                ("README.md", b"readme"),
            ],
        );

        let registry = NodeRegistry::new();
        let provider = crate::LocalFs::new(registry.clone());
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let request = RequestId::new();
        let descriptor = crate::LocationDescriptor::local(&archive).archive_member("");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::descriptor_only(descriptor),
                session,
                pipeline: PipelineConfig::default()
                    .filter(FilterConfig {
                        show_hidden: true,
                        ..FilterConfig::only_extensions(vec!["rs".into()])
                    })
                    .group_by(GroupBy::Extension),
                load: snapshot_load(),
                request,
            })
            .unwrap();

        let groups = wait_for_location_dir_loaded(&evt_rx, session).await;

        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups.len(), 1);
        assert_eq!(groups.groups[0].label, "rs");
        let nodes = &groups.groups[0].nodes;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "main.rs");
        assert!(nodes[0].capabilities.read);
        assert_eq!(
            nodes[0].location.descriptor(),
            Some(&crate::LocationDescriptor::local(&archive).archive_member("main.rs"))
        );
    }

    #[tokio::test]
    async fn test_archive_provider_cache_hit_preserves_member_locations() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("bundle.zip");
        write_zip(&archive, &[("README.md", b"readme")]);

        let registry = NodeRegistry::new();
        let local = Arc::new(crate::LocalFs::new(registry.clone()));
        let provider = Arc::new(crate::ArchiveFs::zip(&archive, local));
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner = Scanner::with_cache(
            cmd_rx,
            evt_tx,
            provider,
            registry,
            cache,
        );
        tokio::spawn(async move { scanner.run().await });

        for _ in 0..2 {
            let session = SessionId::new();
            cmd_tx
                .send(ScanCommand::ScanLocation {
                    location: location_ref(PathBuf::new()),
                    session,
                    pipeline: default_pipeline(),
                    load: snapshot_load(),
                    request: RequestId::new(),
                })
                .unwrap();

            let groups = wait_for_dir_loaded(&evt_rx, session).await;
            assert_eq!(groups.total_count, 1);
            assert_eq!(
                groups.groups[0].nodes[0].location.descriptor(),
                Some(&crate::LocationDescriptor::local(&archive).archive_member("README.md"))
            );
        }
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/progress-fail")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/page")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/page-cursor")),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();
        let (_, first_page) = wait_for_dir_page_loaded(&evt_rx, session).await;
        let next_cursor = first_page.next_cursor.expect("first page should continue");

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/page-cursor")),
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
