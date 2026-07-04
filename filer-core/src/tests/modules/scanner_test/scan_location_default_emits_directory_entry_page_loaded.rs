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
    async fn test_snapshot_and_paged_loads_share_pipeline_order() {
        let provider = MockProvider::new();
        provider.add_file(make_file("z.txt", "/tmp/shared-order", 30, false));
        provider.add_file(_make_file_with_ext(
            "Makefile",
            "/tmp/shared-order",
            None,
            20,
        ));
        provider.add_file(make_file("a.txt", "/tmp/shared-order", 10, false));
        provider.add_file(make_file("same.rs", "/tmp/shared-order/b", 40, false));
        provider.add_file(make_file("same.rs", "/tmp/shared-order/a", 50, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let path = PathBuf::from("/tmp/shared-order");
        let pipeline =
            PipelineConfig::default().sort(SortField::Extension, SortOrder::Ascending, false);

        let snapshot_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: snapshot_session,
                pipeline: pipeline.clone(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let snapshot = wait_for_dir_loaded(&evt_rx, snapshot_session).await;
        let snapshot_names = snapshot.groups[0]
            .nodes
            .iter()
            .map(|node| node.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        let paged_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session: paged_session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::page(2),
                request: RequestId::new(),
            })
            .unwrap();
        let (first, state) = wait_for_dir_page_loaded(&evt_rx, paged_session).await;
        let mut paged_names = first.groups[0]
            .nodes
            .iter()
            .map(|node| node.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let mut next_cursor = state.next_cursor.clone();

        let mut state = state;
        while let Some(cursor) = next_cursor {
            cmd_tx
                .send(ScanCommand::Scan {
                    path: path.clone(),
                    session: paged_session,
                    pipeline: pipeline.clone(),
                    load: crate::DirectoryLoadOptions::page_after(2, cursor),
                    request: RequestId::new(),
                })
                .unwrap();
            let (page, page_state) = wait_for_dir_page_loaded(&evt_rx, paged_session).await;
            paged_names.extend(
                page.groups[0]
                    .nodes
                    .iter()
                    .map(|node| node.path.to_string_lossy().into_owned()),
            );
            next_cursor = page_state.next_cursor.clone();
            state = page_state;
        }

        assert!(state.complete);
        assert_eq!(snapshot_names, paged_names);
        assert_eq!(
            snapshot_names,
            vec![
                "/tmp/shared-order/Makefile",
                "/tmp/shared-order/a/same.rs",
                "/tmp/shared-order/b/same.rs",
                "/tmp/shared-order/a.txt",
                "/tmp/shared-order/z.txt"
            ]
        );
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
