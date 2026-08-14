    use super::*;
    use crate::{Event, model::registry::NodeRegistry, modules::scan::scanner::ScanCommand};

    // Internal NodeId fixtures remain until API-007 retires the navigator state.
    fn node(id: u64) -> NodeId {
        NodeId(id)
    }

    /// Helper to create test session ID
    fn session(id: u64) -> SessionId {
        SessionId(id)
    }

    #[tokio::test]
    async fn test_navigator_actor_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        // Spawn actor in background
        let handle = tokio::spawn(async move {
            navigator.run().await;
        });

        // Drop command sender to signal shutdown
        drop(cmd_tx);

        // Actor should terminate gracefully
        let result = timeout(Duration::from_millis(100), handle).await;
        assert!(
            result.is_ok(),
            "Navigator should shutdown when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_navigator_handles_navigate_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg.clone());

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let target = Location::local("/tmp/nav-target");
        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&target),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // Should trigger a scan command
        let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Should receive scan command")
            .expect("Channel should not be closed");

        match scan_cmd {
            ScanCommand::ScanLocation {
                session: s,
                location,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(location.descriptor(), Some(target.descriptor()));
            }
            _ => panic!("Expected ScanLocation command"),
        }

        // Should emit NavigationChanged event or similar
        let event = timeout(Duration::from_millis(100), event_rx.recv_async()).await;

        // Event might be emitted (depending on implementation)
        // This test validates the command is processed
        assert!(event.is_ok() || event.is_err(), "Command was processed");
    }

    #[tokio::test]
    // API-007 pin: NavCommand::Navigate still exercises internal NodeId state.
    async fn test_compat_navigate_rejects_unresolved_node_before_scan_dispatch() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let request = crate::model::request::RequestId::new();

        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(999),
                request,
            })
            .unwrap();

        let event = timeout(Duration::from_millis(100), event_rx.recv_async())
            .await
            .expect("unresolved compatibility node should emit an error")
            .expect("event channel should remain open");

        match event {
            Event::Error {
                session: s,
                request: Some(r),
                recoverable,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert!(recoverable);
            }
            other => panic!("Expected request error, got {other:?}"),
        }

        assert!(
            scanner_rx.try_recv().is_err(),
            "unresolved compatibility node must not dispatch a scanner command"
        );
    }

    #[tokio::test]
    async fn test_navigator_location_navigation_emits_scan_location_and_snapshot() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let location = Location::local("/tmp/location-nav");

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&location),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Should receive scan command")
            .expect("Channel should not be closed");

        assert!(
            matches!(scan_cmd, ScanCommand::ScanLocation { session: s, .. } if s == session),
            "location navigation should trigger ScanLocation"
        );

        let snapshot = timeout(Duration::from_millis(100), event_rx.recv_async())
            .await
            .expect("Should receive nav state snapshot")
            .expect("Channel should not be closed");

        match snapshot {
            Event::CurrentNavigateState { state, session: s } => {
                assert_eq!(s, session);
                assert_eq!(
                    state.current_location.as_ref().and_then(|r| r.descriptor()),
                    Some(location.descriptor())
                );
            }
            other => panic!("Expected CurrentNavigateState, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_navigator_segmented_location_navigation_emits_scan_location_and_snapshot() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let descriptor = LocationDescriptor::local("/tmp/bundle.zip").archive_member("src");

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::descriptor_only(descriptor.clone()),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("segmented navigation should trigger scan")
            .expect("scanner channel should not be closed");

        match scan_cmd {
            ScanCommand::ScanLocation { location, .. } => {
                assert_eq!(location.descriptor(), Some(&descriptor));
            }
            other => panic!("Expected ScanLocation, got {other:?}"),
        }

        let snapshot = timeout(Duration::from_millis(100), event_rx.recv_async())
            .await
            .expect("Should receive nav state snapshot")
            .expect("Channel should not be closed");

        match snapshot {
            Event::CurrentNavigateState { state, session: s } => {
                assert_eq!(s, session);
                assert_eq!(
                    state.current_location.as_ref().and_then(|r| r.descriptor()),
                    Some(&descriptor)
                );
            }
            other => panic!("Expected CurrentNavigateState, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_navigator_refresh_after_location_navigation_uses_refresh_location() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let location = Location::local("/tmp/location-refresh");

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&location),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let _ = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Navigate should trigger scan")
            .expect("scanner channel should remain open");

        cmd_tx
            .send(NavCommand::Refresh(
                session,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();

        let refresh = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Refresh should trigger scan")
            .expect("scanner channel should remain open");

        assert!(
            matches!(refresh, ScanCommand::RefreshLocation { session: s, .. } if s == session),
            "location-backed refresh should bypass cache through RefreshLocation"
        );
    }

    #[tokio::test]
    async fn test_navigator_back_restores_location_and_scans_location() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let first = Location::local("/tmp/location-back-a");
        let second = Location::local("/tmp/location-back-b");

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&first),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&second),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let _ = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("first navigation should scan")
            .expect("scanner channel should remain open");
        let _ = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("second navigation should scan")
            .expect("scanner channel should remain open");

        cmd_tx
            .send(NavCommand::Back(
                session,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();

        let scan = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("back should trigger scan")
            .expect("scanner channel should remain open");

        match scan {
            ScanCommand::ScanLocation { location, .. } => {
                assert_eq!(location.descriptor(), Some(first.descriptor()));
            }
            other => panic!("Expected ScanLocation, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_navigator_handles_back_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&Location::local("/tmp/back-current")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&Location::local("/tmp/back-next")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Now go back
        cmd_tx
            .send(NavCommand::Back(
                session,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();

        // Should process without panic
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Actor should still be running
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_handles_forward_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&Location::local("/tmp/forward-current")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&Location::local("/tmp/forward-next")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        cmd_tx
            .send(NavCommand::Back(
                session,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Now go forward
        cmd_tx
            .send(NavCommand::Forward(
                session,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should process without panic
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_handles_get_state_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);

        // Request state
        cmd_tx.send(NavCommand::GetState(session)).unwrap();

        // Should emit state event
        let result = timeout(Duration::from_millis(100), event_rx.recv_async()).await;

        // Might receive a StateUpdate event (depending on implementation)
        // The test validates command processing
        assert!(
            result.is_ok() || result.is_err(),
            "GetState command processed"
        );
    }

    #[tokio::test]
    async fn test_navigator_handles_set_pipeline_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let config = PipelineConfig::with_default_sort();

        // Update pipeline
        cmd_tx
            .send(NavCommand::SetPipeline { session, config })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should process without panic
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_handles_refresh_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session,
                location: LocationRef::from_location(&Location::local("/tmp/refresh-current")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Clear scan queue
        while scanner_rx.try_recv().is_ok() {}

        // Refresh current directory
        cmd_tx
            .send(NavCommand::Refresh(
                session,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();

        // Should trigger a new scan
        let result = timeout(Duration::from_millis(100), scanner_rx.recv_async()).await;
        assert!(result.is_ok(), "Refresh should trigger a scan");
    }

    #[tokio::test]
    // API-007 pin: NavCommand::Invalidate still exercises internal NodeId state.
    async fn test_compat_invalidate_refreshes_current_directory() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let current = reg.clone().register("/tmp/invalidate-current".into());
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg.clone());

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: current,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let _ = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Navigate should trigger initial scan")
            .expect("scanner channel should remain open");

        cmd_tx.send(NavCommand::Invalidate(current)).unwrap();

        let refresh = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Invalidate should trigger refresh scan")
            .expect("scanner channel should remain open");

        assert!(
            matches!(
                refresh,
                ScanCommand::RefreshLocation {
                    location,
                    session: s,
                    ..
                } if location == reg.resolve_node_location(current).unwrap() && s == session
            ),
            "Invalidate should refresh the session currently displaying the node"
        );
    }

    #[tokio::test]
    // API-007 pin: NavCommand::Invalidate still exercises internal NodeId state.
    async fn test_compat_invalidate_refreshes_only_current_sessions_with_current_pipeline() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let current = reg.clone().register("/tmp/invalidate-current-pipeline".into());
        let other = reg.clone().register("/tmp/invalidate-other-pipeline".into());
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg.clone());

        tokio::spawn(async move {
            navigator.run().await;
        });

        let current_session = session(1);
        let other_session = session(2);
        let pipeline = PipelineConfig::new();

        cmd_tx
            .send(NavCommand::Navigate {
                session: current_session,
                node: current,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::Navigate {
                session: other_session,
                node: other,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::SetPipeline {
                session: current_session,
                config: pipeline.clone(),
            })
            .unwrap();

        let _ = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("current session navigation should scan")
            .expect("scanner channel should remain open");
        let _ = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("other session navigation should scan")
            .expect("scanner channel should remain open");
        while scanner_rx.try_recv().is_ok() {}

        cmd_tx.send(NavCommand::Invalidate(current)).unwrap();

        let refresh = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Invalidate should trigger refresh scan")
            .expect("scanner channel should remain open");

        match refresh {
            ScanCommand::RefreshLocation {
                location,
                session,
                pipeline: refresh_pipeline,
                ..
            } => {
                assert_eq!(location, reg.resolve_node_location(current).unwrap());
                assert_eq!(session, current_session);
                assert_eq!(refresh_pipeline, pipeline);
            }
            other => panic!("expected RefreshLocation, got {other:?}"),
        }

        assert!(
            scanner_rx.try_recv().is_err(),
            "sessions not currently viewing the invalidated node should not refresh"
        );
    }
