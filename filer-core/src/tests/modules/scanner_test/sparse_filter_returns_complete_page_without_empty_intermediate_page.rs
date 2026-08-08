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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-empty-continue")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-cancel")),
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
            matches!(event, Event::DirectoryPageLoaded { session: s, .. } if *s == session)
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-mutation")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-mutation")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-mismatch")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-mismatch")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-cache")),
                session: snapshot_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, snapshot_session).await;

        let filter_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(PathBuf::from("/tmp/filter-cache")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(path.clone()),
                session: s1,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(path.clone()),
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
    async fn test_same_folder_scan_serves_cached_listing() {
        let provider = MockProvider::new();
        provider.add_file(make_file("before.txt", "/tmp/same-folder-cache", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/same-folder-cache");
        let first_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(path.clone()),
                session: first_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let first_groups = wait_for_dir_loaded(&evt_rx, first_session).await;
        assert_eq!(first_groups.total_count, 1);

        provider.add_file(make_file("after.txt", "/tmp/same-folder-cache", 20, false));

        let second_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(path),
                session: second_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let second_groups = wait_for_dir_loaded(&evt_rx, second_session).await;

        assert_eq!(
            provider.get_list_calls().len(),
            1,
            "same-folder scans should reuse the complete cached listing"
        );
        assert_eq!(
            second_groups.total_count, 1,
            "same-folder scans should emit the cached listing until refreshed"
        );
        assert_eq!(second_groups.groups[0].nodes[0].name, "before.txt");
    }
