    // Compatibility pin for API-006: preserve NodeId navigation routing until
    // the compatibility command is removed and replaced with an absence test.
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

    // Compatibility pin for API-006: preserve NodeId search resolution and
    // compatibility event mode until the route is removed.
    #[tokio::test]
    async fn test_route_search_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // The registry is required because this compatibility route still accepts NodeId.
        let path = PathBuf::from("/home/user/projects");
        let registered_id = harness.registry.clone().register(path);

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
                root,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(query.text, "*.rs", "Query text must be forwarded");
                assert_eq!(
                    root,
                    harness.registry.resolve_node_location(registered_id).unwrap(),
                    "NodeId must resolve to LocationRef before reaching the searcher"
                );
                assert_eq!(event_mode, SearchEventMode::Compat);
                assert_eq!(s, session, "Session must be the same for both command");
            }
            other => panic!("Expected SearchCommand::Search, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve path and NodeId scan routes and
    // their shared compatibility event behavior until both routes are removed.
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
            ScanCommand::ScanCompat {
                location,
                session: routed_session,
                pipeline: routed_pipeline,
                load: routed_load,
                request,
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path.clone()))
                );
                assert_eq!(routed_session, session);
                assert_eq!(routed_pipeline, pipeline);
                assert_eq!(routed_load, load);
                assert_eq!(request, path_request);
            }
            other => panic!("Expected ScanCommand::ScanCompat, got {other:?}"),
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
            ScanCommand::ScanCompat {
                location,
                session: routed_session,
                pipeline: routed_pipeline,
                load: routed_load,
                request,
            } => {
                assert_eq!(location, harness.registry.resolve_node_location(node).unwrap());
                assert_eq!(routed_session, session);
                assert_eq!(routed_pipeline, pipeline);
                assert_eq!(routed_load, load);
                assert_eq!(request, node_request);
            }
            other => panic!("Expected ScanCommand::ScanCompat, got {other:?}"),
        }
    }

    // Compatibility pin for API-006: preserve NodeId watch resolution and
    // compatibility event mode until the route is removed.
    #[tokio::test]
    async fn test_route_watch_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/watched");
        let node = harness.registry.clone().register(path);

        harness
            .send(Command::WatchNodeCompat { node, session })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::Watch {
                location,
                session: s,
                request,
                event_mode,
            } => {
                assert_eq!(
                    location,
                    harness.registry.resolve_node_location(node).unwrap(),
                    "NodeId must resolve before reaching the watcher"
                );
                assert_eq!(s, session, "Watch SessionId must be forwarded");
                assert!(request.is_none(), "compat watch has no request id");
                assert_eq!(event_mode, WatchEventMode::Compat { node });
            }
            other => panic!("Expected WatchCommand::Watch, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId unwatch resolution until
    // the compatibility route is removed.
    #[tokio::test]
    async fn test_route_unwatch_to_watcher() {
        let harness = RouterTestHarness::new();
        let path = PathBuf::from("/home/user/unwatched");
        let node = harness.registry.clone().register(path);

        // The compatibility command carries only NodeId and no SessionId.
        harness.send(Command::UnwatchNodeCompat { node }).await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::Unwatch { location, scope } => {
                assert_eq!(
                    location,
                    harness.registry.resolve_node_location(node).unwrap(),
                    "NodeId must resolve before reaching the watcher"
                );
                assert_eq!(scope, UnwatchScope::All);
            }
            other => panic!("Expected WatchCommand::Unwatch, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId preview routing and the
    // option forwarding contract until the route is removed.
    #[tokio::test]
    async fn test_route_load_preview_with_options() {
        use crate::PreviewOptions;

        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/image.png");
        let node = harness.registry.clone().register(path.clone());

        let options = PreviewOptions {
            max_width: 800,
            max_height: 600,
            ..PreviewOptions::default()
        };

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: node,
                options: Some(options.clone()),
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match preview_cmd {
            PreviewCommand::Generate {
                location,
                options: o,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "NodeId must resolve to LocationRef before preview generation"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(
                    s, session,
                    "Preview request session id must match requested command"
                );
                assert!(o.is_some(), "Options should be forwarded");
                let opts = o.unwrap();
                assert_eq!(opts.max_width, 800);
                assert_eq!(opts.max_height, 600);
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId metadata resolution until
    // the compatibility route is removed.
    #[tokio::test]
    async fn test_route_load_metadata_node_compat_resolves_location() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/document.pdf");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadMetadataNodeCompat {
                node,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadMetadata {
                location,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "NodeId must resolve to LocationRef before metadata loading"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session);
            }
            other => panic!("Expected PreviewCommand::LoadMetadata, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId extended-metadata routing
    // until the compatibility route is removed.
    #[tokio::test]
    async fn test_route_load_extended_metadata_node_compat_resolves_location() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/music.mp3");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadExtendedMetadataNodeCompat {
                node,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadExtendedMetadata {
                location,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "NodeId must resolve to LocationRef before extended metadata loading"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session);
            }
            other => panic!(
                "Expected PreviewCommand::LoadExtendedMetadata, got {:?}",
                other
            ),
        }
    }

    // Compatibility pin for API-006: preserve unresolved NodeId preview
    // behavior so removal can become an explicit absence test.
    #[tokio::test]
    async fn test_route_unresolved_preview_node_compat_emits_error() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: NodeId(404),
                options: None,
                session,
                request,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        assert!(matches!(
            event,
            Event::Error {
                session: s,
                request: Some(r),
                ..
            } if s == session && r == request
        ));
        assert!(harness.preview_rx.try_recv().is_err());
    }

    // Compatibility pin for API-006: preserve NodeId preview resolution until
    // the compatibility route is removed.
    #[tokio::test]
    async fn test_route_load_preview_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/photo.jpg");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: node,
                options: None,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::Generate {
                location,
                options,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "NodeId must resolve to LocationRef before preview generation"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert!(options.is_none(), "Options should be None when not provided");
                assert_eq!(s, session, "Preview session id must match request command");
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId metadata resolution until
    // the compatibility route is removed.
    #[tokio::test]
    async fn test_route_load_metadata_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/document.pdf");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadMetadataNodeCompat {
                node,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadMetadata {
                location,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "NodeId must resolve to LocationRef before metadata loading"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session, "Load request session must match the command");
            }
            other => panic!("Expected PreviewCommand::LoadMetadata, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId extended-metadata
    // resolution until the compatibility route is removed.
    #[tokio::test]
    async fn test_route_load_extended_metadata_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/music.mp3");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadExtendedMetadataNodeCompat {
                node,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadExtendedMetadata {
                location,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "NodeId must resolve to LocationRef before extended metadata loading"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session, "Extended metadata session must match request");
            }
            other => panic!(
                "Expected PreviewCommand::LoadExtendedMetadata, got {:?}",
                other
            ),
        }
    }

    // Compatibility pin for API-006: preserve unresolved NodeId operation
    // behavior so API-006 can turn it into an absence test.
    #[tokio::test]
    async fn test_route_unresolved_copy_node_compat_emits_error() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let missing = NodeId::from_path(&PathBuf::from("/missing/source.txt"));
        let destination = harness.registry.clone().register(PathBuf::from("/b"));
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CopyNodeCompat {
                sources: vec![missing],
                destination,
                session,
                request,
                operation,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                session: s,
                request: r,
                operation: op,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(r, Some(request));
                assert_eq!(op, Some(operation));
            }
            other => panic!("Expected Event::Error, got {other:?}"),
        }

        assert!(
            harness.ops_rx.try_recv().is_err(),
            "unresolved compat nodes must not reach the operator"
        );
    }
