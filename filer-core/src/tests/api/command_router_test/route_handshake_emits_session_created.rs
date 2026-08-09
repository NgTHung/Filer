    #[tokio::test]
    async fn test_route_handshake_emits_session_created() {
        let harness = RouterTestHarness::new();

        harness.send(Command::Handshake).await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for SessionCreated event")
            .expect("Event channel closed");

        match event {
            Event::SessionCreated(s) => {
                // A new session ID should have been generated
                assert_ne!(
                    s,
                    SessionId::DEFAULT,
                    "Generated session should not be DEFAULT"
                );
                // Session should now exist in the manager
                assert!(
                    harness.session_manager.exists(s),
                    "Session must exist after Handshake"
                );
            }
            other => panic!("Expected SessionCreated, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_destroy_session_cleans_up_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::DestroySession(session)).await;

        // DestroySession should send both UnwatchSession to watcher
        // and RemoveSession to navigator
        let _watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand::RemoveSession")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::RemoveSession(s) => {
                assert_eq!(s, session);
            }
            other => panic!("Expected NavCommand::RemoveSession, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_destroy_session_emits_event() {
        let harness = RouterTestHarness::new();
        // Must create a valid session first
        let session = harness.create_valid_session();

        harness.send(Command::DestroySession(session)).await;

        // DestroySession should send UnwatchSession to watcher
        let _watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        // And RemoveSession to navigator
        let _nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand::RemoveSession")
            .expect("NavCommand channel closed");

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for SessionDestroyed event")
            .expect("Event channel closed");

        match event {
            Event::SessionDestroyed(s) => {
                assert_eq!(
                    s, session,
                    "SessionDestroyed must carry the correct session"
                );
                // Session should be removed from manager
                assert!(
                    !harness.session_manager.exists(session),
                    "Session must not exist after DestroySession"
                );
            }
            other => panic!("Expected SessionDestroyed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_destroy_session_unwatches_all_for_session() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::DestroySession(session)).await;

        // The router should send UnwatchSession to watcher to clean up
        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand::UnwatchSession")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::UnwatchSession(s) => {
                assert_eq!(
                    s, session,
                    "UnwatchSession must carry the destroyed session"
                );
            }
            other => panic!("Expected WatchCommand::UnwatchSession, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_unknown_session_navigate_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new(); // Not registered in SessionManager

        harness
            .send(Command::Navigate {
                location: LocationRef::from_location(&Location::local("/home")),
                session: unknown,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                message,
                recoverable,
                session,
                ..
            } => {
                assert_eq!(session, unknown, "Error must carry the unknown session");
                assert!(recoverable, "Unknown session should be a recoverable error");
                assert!(
                    message.contains("Unknown session"),
                    "Error message should mention unknown session, got: {}",
                    message
                );
            }
            other => panic!("Expected Event::Error, got {:?}", other),
        }

        // Navigator should NOT have received anything
        let nav_result = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(
            nav_result.is_err(),
            "Navigator should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_location_command_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();
        let location = Location::local("/tmp/location-api-test");
        let request = RequestId::new();

        harness
            .send(Command::Navigate {
                location: LocationRef::from_location(&location),
                session: unknown,
                request,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                session,
                request: error_request,
                recoverable,
                message,
                ..
            } => {
                assert_eq!(session, unknown, "Error must carry the unknown session");
                assert_eq!(
                    error_request,
                    Some(request),
                    "Error must preserve the request id"
                );
                assert!(recoverable, "Unknown session should be recoverable");
                assert!(
                    message.contains("Unknown session"),
                    "Error message should mention unknown session, got: {}",
                    message
                );
            }
            other => panic!("Expected Event::Error, got {:?}", other),
        }

        let nav_result = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(
            nav_result.is_err(),
            "Navigator should not receive Location command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_search_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();

        harness
            .send(Command::Search {
                query: "*.txt".to_string(),
                root: LocationRef::from_location(&Location::local("/tmp")),
                session: unknown,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error { session, .. } => {
                assert_eq!(session, unknown, "Error must carry the unknown session");
            }
            other => panic!("Expected Event::Error for unknown session, got {:?}", other),
        }

        // Searcher should NOT have received anything
        let search_result =
            timeout(Duration::from_millis(50), harness.search_rx.recv_async()).await;
        assert!(
            search_result.is_err(),
            "Searcher should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_watch_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();

        harness
            .send(Command::Watch {
                location: LocationRef::from_location(&Location::local("/watched")),
                session: unknown,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                session,
                recoverable,
                message,
                ..
            } => {
                assert_eq!(session, unknown);
                assert!(recoverable);
                assert!(message.contains("Unknown session"));
            }
            other => panic!("Expected Event::Error, got {:?}", other),
        }

        // Watcher should NOT have received anything
        let watch_result = timeout(Duration::from_millis(50), harness.watch_rx.recv_async()).await;
        assert!(
            watch_result.is_err(),
            "Watcher should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_ops_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CreateFolder {
                parent: LocationRef::from_location(&Location::local("/home")),
                name: "new_dir".to_string(),
                session: unknown,
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
                session,
                request: err_request,
                operation: err_operation,
                ..
            } => {
                assert_eq!(session, unknown);
                assert_eq!(err_request, Some(request));
                assert_eq!(err_operation, Some(operation));
            }
            other => panic!("Expected Event::Error for unknown session, got {:?}", other),
        }

        // Operator should NOT have received anything
        let ops_result = timeout(Duration::from_millis(50), harness.ops_rx.recv_async()).await;
        assert!(
            ops_result.is_err(),
            "Operator should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_commands_work_after_handshake() {
        let harness = RouterTestHarness::new();

        // Do a proper handshake to get a valid session
        harness.send(Command::Handshake).await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        let session = match event {
            Event::SessionCreated(s) => s,
            other => panic!("Expected SessionCreated, got {:?}", other),
        };

        // Now use that session to navigate
        let location = LocationRef::from_location(&Location::local("/home"));
        harness
            .send(Command::Navigate {
                location,
                session,
                request: RequestId::new(),
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for nav command")
            .expect("Nav channel closed");

        match nav_cmd {
            NavCommand::NavigateToLocation {
                session: s,
                location,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(location, LocationRef::from_location(&Location::local("/home")));
            }
            other => panic!("Expected NavigateToLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_commands_fail_after_destroy() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // Verify it works before destroy
        harness
            .send(Command::Navigate {
                location: LocationRef::from_location(&Location::local("/before")),
                session,
                request: RequestId::new(),
            })
            .await;
        let _ = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async()).await;

        // Destroy the session
        harness.send(Command::DestroySession(session)).await;
        // Drain destroy hooks: UnwatchSession, RemoveSession, and SessionDestroyed
        let _ = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async()).await;
        let _ = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async()).await;
        let _ = timeout(TEST_TIMEOUT, harness.event_rx.recv_async()).await;

        // Now try to use the destroyed session
        harness
            .send(Command::Navigate {
                location: LocationRef::from_location(&Location::local("/after")),
                session,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match event {
            Event::Error { session: s, .. } => {
                assert_eq!(s, session, "Error must reference the destroyed session");
            }
            other => panic!("Expected Event::Error after destroy, got {:?}", other),
        }

        // Navigator should NOT receive the post-destroy command
        let nav_result = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(
            nav_result.is_err(),
            "Navigator should not receive commands for destroyed session"
        );
    }

    #[tokio::test]
    async fn test_router_shuts_down_when_command_channel_closes() {
        let (command_tx, command_rx) = flume::unbounded::<Command>();
        let (event_tx, _event_rx) = flume::unbounded::<Event>();
        let registry = NodeRegistry::new();
        let session_manager = SessionManager::new(registry.clone());

        let handlers = Arc::new(HandlerRegistry::new());
        let ctx = HandlerContext {
            events: event_tx,
            sessions: session_manager,
            registry,
        };

        let router = CommandRouter::new(command_rx, handlers, ctx);
        let handle = tokio::spawn(async move { router.run().await });

        // Drop sender to close the command channel
        drop(command_tx);

        // Router should exit gracefully
        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(
            result.is_ok(),
            "Router should exit when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_router_processes_commands_sequentially() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // Send multiple navigation commands rapidly
        for i in 0..5 {
            harness
                .send(Command::Navigate {
                    location: LocationRef::from_location(&Location::local(format!("/dir/{}", i))),
                    session,
                    request: RequestId::new(),
                })
                .await;
        }

        // All 5 should arrive in order
        for i in 0..5 {
            let cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
                .await
                .expect("Timed out")
                .expect("Channel closed");
            match cmd {
                NavCommand::NavigateToLocation {
                    location,
                    session: s,
                    ..
                } => {
                    assert_eq!(s, session);
                    assert_eq!(
                        location,
                        LocationRef::from_location(&Location::local(format!("/dir/{}", i)))
                    );
                }
                other => panic!("Expected NavigateToLocation, got {:?}", other),
            }
        }
    }

    #[tokio::test]
    async fn test_navigate_does_not_reach_searcher_or_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness
            .send(Command::Navigate {
                location: LocationRef::from_location(&Location::local("/test")),
                session,
                request: RequestId::new(),
            })
            .await;

        // Navigator gets the command
        let nav = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async()).await;
        assert!(nav.is_ok(), "Navigator should receive the command");

        // Searcher should NOT get anything
        let search = timeout(Duration::from_millis(50), harness.search_rx.recv_async()).await;
        assert!(
            search.is_err(),
            "Searcher should not receive Navigate commands"
        );

        // Watcher should NOT get anything
        let watch = timeout(Duration::from_millis(50), harness.watch_rx.recv_async()).await;
        assert!(
            watch.is_err(),
            "Watcher should not receive Navigate commands"
        );

        // Previewer should NOT get anything
        let preview = timeout(Duration::from_millis(50), harness.preview_rx.recv_async()).await;
        assert!(
            preview.is_err(),
            "Previewer should not receive Navigate commands"
        );
    }

    #[tokio::test]
    async fn test_search_does_not_reach_navigator_or_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness
            .send(Command::Search {
                query: "test".to_string(),
                root: LocationRef::from_location(&Location::local("/root")),
                session,
                request: RequestId::new(),
            })
            .await;

        // Searcher gets the command
        let search = timeout(TEST_TIMEOUT, harness.search_rx.recv_async()).await;
        assert!(search.is_ok(), "Searcher should receive the command");

        // Navigator should NOT get anything
        let nav = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(nav.is_err(), "Navigator should not receive Search commands");

        // Watcher should NOT get anything
        let watch = timeout(Duration::from_millis(50), harness.watch_rx.recv_async()).await;
        assert!(watch.is_err(), "Watcher should not receive Search commands");
    }

    #[tokio::test]
    async fn test_watch_does_not_reach_navigator_or_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness
            .send(Command::Watch {
                location: LocationRef::from_location(&Location::local("/watched")),
                session,
                request: RequestId::new(),
            })
            .await;

        // Watcher gets the command
        let watch = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async()).await;
        assert!(watch.is_ok(), "Watcher should receive the command");

        // Navigator should NOT get anything
        let nav = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(nav.is_err(), "Navigator should not receive Watch commands");

        // Searcher should NOT get anything
        let search = timeout(Duration::from_millis(50), harness.search_rx.recv_async()).await;
        assert!(
            search.is_err(),
            "Searcher should not receive Watch commands"
        );
    }
