    #[tokio::test]
    async fn test_route_navigate_path_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/documents");

        let request = RequestId::new();
        harness
            .send(Command::NavigatePathCompat {
                path: path.clone(),
                session,
                request,
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::NavigateToPath {
                session: s,
                path: p,
                request: r,
            } => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(p, path, "Path must be forwarded correctly");
                assert_eq!(r, request, "RequestId must be forwarded correctly");
            }
            other => panic!("Expected NavCommand::NavigateToPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_navigate_location_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::Navigate {
                location: location_ref.clone(),
                session,
                request,
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::NavigateToLocation {
                session: s,
                location,
                request: r,
            } => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected NavCommand::NavigateToLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_navigate_to_node_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let node = NodeId(42);

        let request = RequestId::new();
        harness
            .send(Command::NavigateNodeCompat {
                node,
                session,
                request,
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::Navigate {
                session: s,
                node: n,
                request: r,
            } => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(n, node, "NodeId must be forwarded correctly");
                assert_eq!(r, request, "RequestId must be forwarded correctly");
            }
            other => panic!("Expected NavCommand::Navigate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_navigate_up_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        let request = RequestId::new();
        harness.send(Command::NavigateUp { session, request }).await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::Up(s, r) => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be preserved");
            }
            other => panic!("Expected NavCommand::Up, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_refresh_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        let request = RequestId::new();
        harness.send(Command::Refresh { session, request }).await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::Refresh(s, r) => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be preserved");
            }
            other => panic!("Expected NavCommand::Refresh, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_search_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // Register a path for the root NodeId so the router can resolve it
        let path = PathBuf::from("/home/user/projects");
        let registered_id = harness.registry.clone().register(path.clone());

        harness
            .send(Command::SearchNodeCompat {
                query: "*.rs".to_string(),
                root: registered_id,
                session,
                request: RequestId::new(),
            })
            .await;

        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand")
            .expect("SearchCommand channel closed");

        match search_cmd {
            SearchCommand::Search {
                query,
                root: r,
                session: s,
                ..
            } => {
                assert_eq!(query.text, "*.rs", "Query text must be forwarded");
                assert_eq!(r, registered_id, "Root NodeId must be forwarded");
                assert_eq!(s, session, "Session must be the same for both command");
            }
            other => panic!("Expected SearchCommand::Search, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_search_path_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();
        let root = PathBuf::from("/home/user/projects");

        harness
            .send(Command::SearchPathCompat {
                query: "*.rs".to_string(),
                root: root.clone(),
                session,
                request,
            })
            .await;

        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand")
            .expect("SearchCommand channel closed");

        match search_cmd {
            SearchCommand::SearchPath {
                query,
                root: r,
                session: s,
                request: req,
            } => {
                assert_eq!(query.text, "*.rs", "Query text must be forwarded");
                assert_eq!(r, root, "Root path must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(req, request, "RequestId must be forwarded");
            }
            other => panic!("Expected SearchCommand::SearchPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_search_location_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::Search {
                query: "report".to_string(),
                root: location_ref.clone(),
                session,
                request,
            })
            .await;

        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand")
            .expect("SearchCommand channel closed");

        match search_cmd {
            SearchCommand::SearchLocation {
                query,
                root,
                session: s,
                request: r,
            } => {
                assert_eq!(query.text, "report", "Query text must be forwarded");
                assert_eq!(root, location_ref, "LocationRef must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected SearchCommand::SearchLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_invalid_search_query_emits_request_error() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::Search {
                query: "type:unknown".to_string(),
                root: LocationRef::from_location(&Location::local("/tmp/search")),
                session,
                request,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for invalid query error")
            .expect("Event channel closed");

        match event {
            Event::Error {
                code,
                session: s,
                request: error_request,
                ..
            } => {
                assert_eq!(code, ErrorCode::InputInvalid);
                assert_eq!(s, session);
                assert_eq!(error_request, Some(request));
            }
            other => panic!("Expected Event::Error, got {other:?}"),
        }

        assert!(
            harness.search_rx.try_recv().is_err(),
            "invalid query must not be forwarded to Searcher"
        );
    }

    #[tokio::test]
    async fn test_route_scan_location_to_scanner() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test");
        let location_ref = LocationRef::from_location(&location);
        let pipeline = PipelineConfig::default();
        let request = RequestId::new();

        harness
            .send(Command::Scan {
                location: location_ref.clone(),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::unbounded(crate::ListingOptions::metadata()),
                request,
            })
            .await;

        let scan_cmd = timeout(TEST_TIMEOUT, harness.scan_rx.recv_async())
            .await
            .expect("Timed out waiting for ScanCommand")
            .expect("ScanCommand channel closed");

        match scan_cmd {
            ScanCommand::ScanLocation {
                location,
                session: s,
                pipeline: p,
                load,
                request: r,
            } => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(p, pipeline, "PipelineConfig must be forwarded");
                assert_eq!(
                    load,
                    crate::DirectoryLoadOptions::unbounded(crate::ListingOptions::metadata()),
                    "DirectoryLoadOptions must be forwarded"
                );
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected ScanCommand::ScanLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_compat_scans_to_scanner() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/tmp/compat-scan");
        let node = harness.registry.clone().register(path.clone());
        let pipeline = PipelineConfig::with_default_sort();
        let load = crate::DirectoryLoadOptions::bounded(32);
        let path_request = RequestId::new();
        let node_request = RequestId::new();

        harness
            .send(Command::ScanPathCompat {
                path: path.clone(),
                session,
                pipeline: pipeline.clone(),
                load: load.clone(),
                request: path_request,
            })
            .await;

        match timeout(TEST_TIMEOUT, harness.scan_rx.recv_async())
            .await
            .expect("Timed out waiting for path ScanCommand")
            .expect("ScanCommand channel closed")
        {
            ScanCommand::Scan {
                path: routed_path,
                session: routed_session,
                pipeline: routed_pipeline,
                load: routed_load,
                request,
            } => {
                assert_eq!(routed_path, path);
                assert_eq!(routed_session, session);
                assert_eq!(routed_pipeline, pipeline);
                assert_eq!(routed_load, load);
                assert_eq!(request, path_request);
            }
            other => panic!("Expected ScanCommand::Scan, got {other:?}"),
        }

        harness
            .send(Command::ScanNodeCompat {
                node,
                session,
                pipeline: pipeline.clone(),
                load: load.clone(),
                request: node_request,
            })
            .await;

        match timeout(TEST_TIMEOUT, harness.scan_rx.recv_async())
            .await
            .expect("Timed out waiting for node ScanCommand")
            .expect("ScanCommand channel closed")
        {
            ScanCommand::ScanNode {
                node: routed_node,
                session: routed_session,
                pipeline: routed_pipeline,
                load: routed_load,
                request,
            } => {
                assert_eq!(routed_node, node);
                assert_eq!(routed_session, session);
                assert_eq!(routed_pipeline, pipeline);
                assert_eq!(routed_load, load);
                assert_eq!(request, node_request);
            }
            other => panic!("Expected ScanCommand::ScanNode, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_route_cancel_to_searcher_when_searching() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // First start a search so the router knows this session is searching
        let root = harness.registry.clone().register(PathBuf::from("/tmp"));
        harness
            .send(Command::SearchNodeCompat {
                query: "test".to_string(),
                root,
                session,
                request: RequestId::new(),
            })
            .await;

        // Drain the search command
        let _ = timeout(TEST_TIMEOUT, harness.search_rx.recv_async()).await;

        // Now cancel
        harness.send(Command::CancelSearch { session }).await;

        let cancel_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand::Cancel")
            .expect("SearchCommand channel closed");

        match cancel_cmd {
            SearchCommand::Cancel(s) => {
                assert_eq!(s, session, "Session must be the same for Cancel request");
            }
            other => panic!("Expected SearchCommand::Cancel, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_watch_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/watched");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::WatchNodeCompat { node, session })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::Watch(n, s) => {
                assert_eq!(n, node, "Watch NodeId must be forwarded");
                assert_eq!(s, session, "Watch SessionId must be forwarded");
            }
            other => panic!("Expected WatchCommand::Watch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_unwatch_to_watcher() {
        let harness = RouterTestHarness::new();
        let path = PathBuf::from("/home/user/unwatched");
        let node = harness.registry.clone().register(path.clone());

        // Unwatch carries only NodeId, no SessionId
        harness.send(Command::UnwatchNodeCompat { node }).await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::Unwatch(n) => {
                assert_eq!(n, node, "Unwatch NodeId must be forwarded");
            }
            other => panic!("Expected WatchCommand::Unwatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_watch_location_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = LocationRef::from_location(&Location::local("/home/user/watched"));
        let request = RequestId::new();

        harness
            .send(Command::Watch {
                location: location.clone(),
                session,
                request,
            })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::WatchLocation {
                location: routed,
                session: s,
                request: r,
            } => {
                assert_eq!(routed, location);
                assert_eq!(s, session);
                assert_eq!(r, request);
            }
            other => panic!("Expected WatchCommand::WatchLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_unwatch_location_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = LocationRef::from_location(&Location::local("/home/user/watched"));

        harness
            .send(Command::Unwatch {
                location: location.clone(),
                session,
            })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::UnwatchLocation {
                location: routed,
                session: s,
            } => {
                assert_eq!(routed, location);
                assert_eq!(s, session);
            }
            other => panic!("Expected WatchCommand::UnwatchLocation, got {:?}", other),
        }
    }
