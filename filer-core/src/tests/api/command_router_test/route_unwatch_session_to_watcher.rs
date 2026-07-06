    #[tokio::test]
    async fn test_route_unwatch_session_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::UnwatchSession(session)).await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::UnwatchSession(s) => {
                assert_eq!(s, session, "UnwatchSession must forward SessionId");
            }
            other => panic!("Expected WatchCommand::UnwatchSession, got {:?}", other),
        }
    }

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
                    "Preview location must resolve from request NodeId"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert!(
                    options.is_none(),
                    "Options should be None when not provided"
                );
                assert_eq!(s, session, "Preview session id must match request command");
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_preview_location_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test/preview.txt");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::LoadPreview {
                location: location_ref.clone(),
                options: None,
                session,
                request,
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
                request: r,
            } => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(event_mode, PreviewEventMode::Location);
                assert!(options.is_none(), "Options must be forwarded as None");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_cancel_preview_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::CancelPreview { session }).await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::Cancel(s) => {
                assert_eq!(s, session, "Cancel preview session must match the command");
            }
            other => panic!("Expected PreviewCommand::Cancel, got {:?}", other),
        }
    }

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
                    "Metadata location must resolve from request NodeId"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session, "Load request session id must match the command");
            }
            other => panic!("Expected PreviewCommand::LoadMetadata, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_metadata_location_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test/document.pdf");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::LoadMetadata {
                location: location_ref.clone(),
                session,
                request,
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
                request: r,
            } => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(event_mode, PreviewEventMode::Location);
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected PreviewCommand::LoadMetadata, got {:?}", other),
        }
    }

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
                    "Extended metadata location must resolve from request NodeId"
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

    #[tokio::test]
    async fn test_route_load_extended_metadata_location_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test/music.mp3");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::LoadExtendedMetadata {
                location: location_ref.clone(),
                session,
                request,
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
                request: r,
            } => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(event_mode, PreviewEventMode::Location);
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected PreviewCommand::LoadExtendedMetadata, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_copy_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let src = harness
            .registry
            .clone()
            .register(PathBuf::from("/a/file.txt"));
        let dst = harness.registry.clone().register(PathBuf::from("/b"));
        let src_location = harness.registry.resolve_node_location(src).unwrap();
        let dst_location = harness.registry.resolve_node_location(dst).unwrap();
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CopyNodeCompat {
                sources: vec![src],
                destination: dst,
                session,
                request,
                operation,
            })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::Copy {
                sources,
                destination,
                event_mode,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(sources, vec![src_location]);
                assert_eq!(destination, dst_location);
                assert_eq!(event_mode, OperationEventMode::Compat);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::Copy, got {:?}", other),
        }
    }

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

    #[tokio::test]
    async fn test_route_create_file_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let parent = harness
            .registry
            .clone()
            .register(PathBuf::from("/home/user"));
        let parent_location = harness.registry.resolve_node_location(parent).unwrap();
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CreateFileNodeCompat {
                parent,
                name: "notes.txt".to_string(),
                session,
                request,
                operation,
            })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::CreateFile {
                parent: p,
                name,
                event_mode,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(p, parent_location);
                assert_eq!(name, "notes.txt");
                assert_eq!(event_mode, OperationEventMode::Compat);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::CreateFile, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_cancel_operation_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let operation = OperationId::new();

        harness
            .send(Command::CancelOperation { session, operation })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::CancelOperation {
                session: s,
                operation: op,
            } => {
                assert_eq!(s, session);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::CancelOperation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_location_write_commands_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let src = LocationRef::from_location(&Location::local("/a/file.txt"));
        let dst = LocationRef::from_location(&Location::local("/b"));
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::Copy {
                sources: vec![src.clone()],
                destination: dst.clone(),
                session,
                request,
                operation,
            })
            .await;

        match timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed")
        {
            OpsCommand::Copy {
                sources,
                destination,
                event_mode,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(sources, vec![src]);
                assert_eq!(destination, dst);
                assert_eq!(event_mode, OperationEventMode::Location);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::Copy, got {:?}", other),
        }
    }

    #[test]
    fn test_location_command_metadata() {
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let location = LocationRef::from_location(&Location::local("/tmp/file.txt"));

        let copy = Command::Copy {
            sources: vec![location.clone()],
            destination: location.clone(),
            session,
            request,
            operation,
        };
        assert_eq!(copy.key(), "ops.copy");
        assert_eq!(copy.session_id(), Some(session));
        assert_eq!(copy.request_id(), Some(request));
        assert_eq!(copy.operation_id(), Some(operation));

        let watch = Command::Watch {
            location,
            session,
            request,
        };
        assert_eq!(watch.key(), "watch");
        assert_eq!(watch.session_id(), Some(session));
        assert_eq!(watch.request_id(), Some(request));
        assert_eq!(watch.operation_id(), None);

        let cancel_operation = Command::CancelOperation { session, operation };
        assert_eq!(cancel_operation.key(), "ops.cancel");
        assert_eq!(cancel_operation.session_id(), Some(session));
        assert_eq!(cancel_operation.request_id(), None);
        assert_eq!(cancel_operation.operation_id(), Some(operation));
    }

    #[tokio::test]
    async fn test_route_preserves_session_id_navigate() {
        let harness = RouterTestHarness::new();
        let session_a = harness.create_valid_session();
        let session_b = harness.create_valid_session();

        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/a"),
                session: session_a,
                request: RequestId::new(),
            })
            .await;
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/b"),
                session: session_b,
                request: RequestId::new(),
            })
            .await;

        // First command should carry session_a
        let cmd1 = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");
        match cmd1 {
            NavCommand::NavigateToPath { session, path, .. } => {
                assert_eq!(session, session_a);
                assert_eq!(path, PathBuf::from("/a"));
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }

        // Second command should carry session_b
        let cmd2 = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");
        match cmd2 {
            NavCommand::NavigateToPath { session, path, .. } => {
                assert_eq!(session, session_b);
                assert_eq!(path, PathBuf::from("/b"));
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_different_sessions_to_different_actors() {
        let harness = RouterTestHarness::new();
        let session_a = harness.create_valid_session();
        let session_b = harness.create_valid_session();

        // Session A navigates
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/home/a"),
                session: session_a,
                request: RequestId::new(),
            })
            .await;

        // Session B searches
        let root = harness
            .registry
            .clone()
            .register(PathBuf::from("/search/root"));
        harness
            .send(Command::SearchNodeCompat {
                query: "find me".to_string(),
                root,
                session: session_b,
                request: RequestId::new(),
            })
            .await;

        // Navigate should go to Navigator
        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for nav")
            .expect("Nav channel closed");
        match nav_cmd {
            NavCommand::NavigateToPath { session, .. } => {
                assert_eq!(session, session_a, "Navigate must carry session A");
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }

        // Search should go to Searcher
        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for search")
            .expect("Search channel closed");
        match search_cmd {
            SearchCommand::Search { .. } => {
                // Correctly routed to searcher
            }
            other => panic!("Expected SearchCommand::Search, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_multiple_sessions_interleaved() {
        let harness = RouterTestHarness::new();
        let session_1 = harness.create_valid_session();
        let session_2 = harness.create_valid_session();
        let session_3 = harness.create_valid_session();

        // Interleave commands from 3 sessions going to the same actor
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/s1"),
                session: session_1,
                request: RequestId::new(),
            })
            .await;
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/s2"),
                session: session_2,
                request: RequestId::new(),
            })
            .await;
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/s3"),
                session: session_3,
                request: RequestId::new(),
            })
            .await;

        // All should arrive at Navigator, in order, with correct session IDs
        let mut received_sessions = Vec::new();
        for _ in 0..3 {
            let cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
                .await
                .expect("Timed out")
                .expect("Channel closed");
            match cmd {
                NavCommand::NavigateToPath { session, .. } => {
                    received_sessions.push(session);
                }
                other => panic!("Expected NavigateToPath, got {:?}", other),
            }
        }

        assert_eq!(received_sessions.len(), 3);
        assert_eq!(received_sessions[0], session_1);
        assert_eq!(received_sessions[1], session_2);
        assert_eq!(received_sessions[2], session_3);
    }
