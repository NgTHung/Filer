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
            .send(ScanCommand::ScanLocation {
                location: location_ref(std::path::PathBuf::from("/tmp/dir")),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(path.clone()),
                session: fast_session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::unbounded(ListingOptions::fast()),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, fast_session).await;

        let metadata_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(path),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(std::path::PathBuf::from("/tmp/bounded")),
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
            .send(ScanCommand::ScanLocation {
                    location: location_ref(path.clone()),
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(path.clone()),
                session: full_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, full_session).await;

        let bounded_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(path),
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
    async fn test_scan_location_emits_directory_loaded_from_provider() {
        let provider = MockProvider::new();
        provider.add_file(make_file("native.txt", "/tmp/native-location", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let location = Location::local("/tmp/native-location");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();

        let groups = wait_for_dir_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "native.txt");
    }

    #[tokio::test]
    async fn test_scan_location_emits_location_directory_loaded() {
        let provider = MockProvider::new();
        provider.add_file(make_file("canonical.txt", "/tmp/canonical-location", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let location = Location::local("/tmp/canonical-location");
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();

        let groups = wait_for_location_dir_loaded(&evt_rx, session).await;
        assert_eq!(groups.total_count, 1);
        assert_eq!(groups.groups[0].nodes[0].name, "canonical.txt");
    }

    #[tokio::test]
    async fn test_scan_location_reuses_cache_populated_by_same_location() {
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
        let groups = wait_for_dir_loaded(&evt_rx, second_session).await;

        assert_eq!(groups.total_count, 1);
        assert_eq!(provider.get_list_calls().len(), 1);
    }

    #[tokio::test]
    async fn test_scan_location_reuses_cache_populated_by_location_ref() {
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
        wait_for_dir_loaded(&evt_rx, first_session).await;

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

        let path_session = SessionId::new();
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location_ref(path),
                session: path_session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let path_groups = wait_for_dir_loaded(&evt_rx, path_session).await;

        assert_eq!(provider.get_list_calls().len(), 2);
        assert_eq!(path_groups.total_count, 2);
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
            .send(ScanCommand::ScanLocation {
                location: location_ref(path.clone()),
                session: s1,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        // Invalidate the cache entry
        cache
            .lock()
            .unwrap()
            .invalidate(crate::Location::local(path.clone()).id());

        let s2 = SessionId::new();
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
            2,
            "provider.list() should be called twice after cache invalidation"
        );
    }

    #[tokio::test]
    async fn test_refresh_location_bypasses_cache_after_location_scan() {
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
            .send(ScanCommand::RefreshLocation {
                location: LocationRef::from_location(&location),
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
            "RefreshLocation should bypass cache populated by ScanLocation"
        );
    }

    #[tokio::test]
    async fn test_refresh_location_bypasses_cache_and_emits_location_events() {
        let provider = MockProvider::new();
        provider.add_file(make_file("before.txt", "/tmp/location-refresh", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));

        let scanner =
            Scanner::with_cache(cmd_rx, evt_tx, Arc::new(provider.clone()), registry, cache);
        tokio::spawn(async move { scanner.run().await });

        let location = Location::local("/tmp/location-refresh");
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
        wait_for_dir_loaded(&evt_rx, s1).await;

        provider.add_file(make_file("after.txt", "/tmp/location-refresh", 20, false));

        cmd_tx
            .send(ScanCommand::RefreshLocation {
                location: LocationRef::from_location(&location),
                session: s2,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: RequestId::new(),
            })
            .unwrap();
        let groups = wait_for_dir_loaded(&evt_rx, s2).await;

        assert_eq!(provider.get_list_calls().len(), 2);
        assert_eq!(groups.total_count, 2);
    }

    #[tokio::test]
    async fn test_refresh_location_invalidates_cache_before_scan() {
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
        let location = LocationRef::from_location(&Location::local(path));
        let s1 = SessionId::new();
        let s2 = SessionId::new();

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: location.clone(),
                session: s1,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        wait_for_dir_loaded(&evt_rx, s1).await;

        provider.add_file(make_file("after.txt", "/tmp/refresh", 20, false));

        cmd_tx
            .send(ScanCommand::RefreshLocation {
                location,
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
            "RefreshLocation should bypass cached directory entries"
        );
        assert_eq!(
            groups.total_count, 2,
            "RefreshLocation should emit the fresh provider listing"
        );
    }
