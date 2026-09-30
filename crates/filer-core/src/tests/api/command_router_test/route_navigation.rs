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
            SearchCommand::Search {
                query,
                root,
                event_mode,
                session: s,
                request: r,
            } => {
                assert_eq!(query.text, "report", "Query text must be forwarded");
                assert_eq!(root, location_ref, "LocationRef must be forwarded");
                assert_eq!(event_mode, SearchEventMode::Location);
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected SearchCommand::Search, got {:?}", other),
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
    async fn test_route_cancel_to_searcher_when_searching() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // First start a native search so the router knows this session is searching.
        harness
            .send(Command::Search {
                query: "test".to_string(),
                root: LocationRef::from_location(&Location::local("/tmp")),
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
            WatchCommand::Watch {
                location: routed,
                session: s,
                request: routed_request,
                event_mode,
            } => {
                assert_eq!(routed, location);
                assert_eq!(s, session);
                assert_eq!(routed_request, Some(request));
                assert_eq!(event_mode, WatchEventMode::Location);
            }
            other => panic!("Expected WatchCommand::Watch, got {:?}", other),
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
            WatchCommand::Unwatch {
                location: routed,
                scope,
            } => {
                assert_eq!(routed, location);
                assert_eq!(scope, UnwatchScope::Session(session));
            }
            other => panic!("Expected WatchCommand::Unwatch, got {:?}", other),
        }
    }
