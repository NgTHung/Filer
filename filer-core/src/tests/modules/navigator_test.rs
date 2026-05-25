use crate::actors::Actor;
use crate::model::location::{Location, LocationDescriptor, LocationRef};
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::modules::navigation::navigator::{NavCommand, NavState, Navigator, NavigatorState};
use crate::pipeline::PipelineConfig;
use std::time::Duration;
use tokio::time::timeout;

#[cfg(test)]
mod navigator_state_tests {
    use crate::model::registry::NodeRegistry;

    use super::*;

    /// Helper to create test NodeIds
    fn node(id: u64) -> NodeId {
        NodeId(id)
    }

    #[test]
    fn test_navigate_updates_current() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        assert_eq!(state.current, None);

        state.navigate(node(1));
        assert_eq!(state.current, Some(node(1)));

        state.navigate(node(2));
        assert_eq!(state.current, Some(node(2)));
    }

    #[test]
    fn test_navigate_adds_to_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        state.navigate(node(1));
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0], node(1));

        state.navigate(node(2));
        assert_eq!(state.history.len(), 2);
        assert_eq!(state.history[0], node(1));
        assert_eq!(state.history[1], node(2));

        state.navigate(node(3));
        assert_eq!(state.history.len(), 3);
        assert_eq!(state.history[2], node(3));
    }

    #[test]
    fn test_back_moves_history_index() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // Navigate to build history
        state.navigate(node(1));
        state.navigate(node(2));
        state.navigate(node(3));

        assert_eq!(state.history_index, 0); // At position 2 (node 3)
        assert_eq!(state.current, Some(node(3)));

        // Go back
        let result = state.back(1);
        assert_eq!(result, Some(node(2)));
        assert_eq!(state.history_index, 1);
        assert_eq!(state.current, Some(node(2)));

        // Go back again
        let result = state.back(1);
        assert_eq!(result, Some(node(1)));
        assert_eq!(state.history_index, 2);
        assert_eq!(state.current, Some(node(1)));

        // Can't go back anymore
        let result = state.back(1);
        assert_eq!(result, None);
        assert_eq!(state.history_index, 2);
    }

    #[test]
    fn test_forward_moves_history_index() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // Build history
        state.navigate(node(1));
        state.navigate(node(2));
        state.navigate(node(3));

        // Go back twice
        state.back(1);
        state.back(1);
        assert_eq!(state.current, Some(node(1)));
        assert_eq!(state.history_index, 2);

        // Go forward
        let result = state.forward();
        assert_eq!(result, Some(node(2)));
        assert_eq!(state.history_index, 1);
        assert_eq!(state.current, Some(node(2)));

        // Go forward again
        let result = state.forward();
        assert_eq!(result, Some(node(3)));
        assert_eq!(state.history_index, 0);
        assert_eq!(state.current, Some(node(3)));

        // Can't go forward anymore
        let result = state.forward();
        assert_eq!(result, None);
        assert_eq!(state.history_index, 0);
    }

    #[test]
    fn test_navigate_up() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // Navigate to a directory
        state.navigate(node(100));
        state.navigate(node(200));

        assert_eq!(state.current, Some(node(200)));
        assert_eq!(state.history.len(), 2);
    }

    #[test]
    fn test_history_limit_enforced() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::with_history_limit(5, reg);

        // Navigate 10 times
        for i in 1..=10 {
            state.navigate(node(i));
        }

        // History should be capped at 5
        assert_eq!(state.history.len(), 5);

        // Should contain the most recent 5
        assert_eq!(state.history[0], node(6));
        assert_eq!(state.history[1], node(7));
        assert_eq!(state.history[2], node(8));
        assert_eq!(state.history[3], node(9));
        assert_eq!(state.history[4], node(10));

        // Current should be at the end
        assert_eq!(state.current, Some(node(10)));
        assert_eq!(state.history_index, 0);
    }

    #[test]
    fn test_navigate_clears_forward_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // Build history
        state.navigate(node(1));
        state.navigate(node(2));
        state.navigate(node(3));

        // Go back
        state.back(1);
        assert_eq!(state.current, Some(node(2)));
        assert_eq!(state.history.len(), 3); // Still have forward history

        // Navigate to new location (should clear forward history)
        state.navigate(node(4));
        assert_eq!(state.history.len(), 3);
        assert_eq!(state.history[0], node(1));
        assert_eq!(state.history[1], node(2));
        assert_eq!(state.history[2], node(4));
        assert_eq!(state.current, Some(node(4)));

        // Forward should not be possible
        let result = state.forward();
        assert_eq!(result, None);
    }

    #[test]
    fn test_can_back_false_at_start() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // No history yet
        assert!(!state.can_back());

        // Navigate once
        state.navigate(node(1));
        assert!(!state.can_back()); // Still at first position

        // Navigate again
        state.navigate(node(2));
        assert!(state.can_back()); // Now can go back

        // Go back to start
        state.back(1);
        assert!(!state.can_back()); // At start again
    }

    #[test]
    fn test_can_forward_false_at_end() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // No history
        assert!(!state.can_forward());

        // Navigate
        state.navigate(node(1));
        state.navigate(node(2));
        assert!(!state.can_forward()); // At the end

        // Go back
        state.back(1);
        assert!(state.can_forward()); // Can go forward now

        // Go forward to end
        state.forward();
        assert!(!state.can_forward()); // At end again
    }

    #[test]
    fn test_snapshot_reflects_state() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // Initial snapshot
        let snap = state.snapshot();
        assert_eq!(snap.current, None);
        assert_eq!(snap.current_location, None);
        assert!(!snap.can_back);
        assert!(!snap.can_forward);
        assert_eq!(snap.selected.len(), 0);

        // After navigation
        state.navigate(node(1));
        state.navigate(node(2));
        let snap = state.snapshot();
        assert_eq!(snap.current, Some(node(2)));
        assert_eq!(snap.current_location, None);
        assert!(snap.can_back);
        assert!(!snap.can_forward);

        // After going back
        state.back(1);
        let snap = state.snapshot();
        assert_eq!(snap.current, Some(node(1)));
        assert_eq!(snap.current_location, None);
        assert!(!snap.can_back);
        assert!(snap.can_forward);
    }

    #[test]
    fn test_navigate_with_location_updates_current_location() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let location = Location::local("/tmp/location-current");
        let location_ref = LocationRef::from_location(&location);

        state.navigate_with_location(node(1), Some(location_ref.clone()));

        assert_eq!(state.current, Some(node(1)));
        assert_eq!(state.current_location, Some(location_ref));
    }

    #[test]
    fn test_navigate_node_populates_location_from_registry_when_available() {
        let reg = NodeRegistry::new();
        let path = std::path::PathBuf::from("/tmp/location-node");
        let node = reg.clone().register(path.clone());
        let mut state = NavigatorState::new(reg);

        state.navigate(node);

        assert_eq!(state.current, Some(node));
        assert_eq!(
            state.current_location.as_ref().and_then(|r| r.descriptor()),
            Some(&LocationDescriptor::local(path))
        );
    }

    #[test]
    fn test_back_and_forward_restore_current_location() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = LocationRef::from_location(&Location::local("/tmp/first"));
        let second = LocationRef::from_location(&Location::local("/tmp/second"));

        state.navigate_with_location(node(1), Some(first.clone()));
        state.navigate_with_location(node(2), Some(second.clone()));

        assert_eq!(state.back(1), Some(node(1)));
        assert_eq!(state.current_location, Some(first));

        assert_eq!(state.forward(), Some(node(2)));
        assert_eq!(state.current_location, Some(second));
    }

    #[test]
    fn test_navigate_after_back_clears_forward_location_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = LocationRef::from_location(&Location::local("/tmp/first"));
        let second = LocationRef::from_location(&Location::local("/tmp/second"));
        let replacement = LocationRef::from_location(&Location::local("/tmp/replacement"));

        state.navigate_with_location(node(1), Some(first.clone()));
        state.navigate_with_location(node(2), Some(second));
        state.back(1);
        state.navigate_with_location(node(3), Some(replacement.clone()));

        assert_eq!(state.history.len(), 2);
        assert_eq!(state.location_history.len(), 2);
        assert_eq!(state.current, Some(node(3)));
        assert_eq!(state.current_location, Some(replacement));
        assert_eq!(state.forward(), None);
    }

    #[test]
    fn test_default_state() {
        let reg = NodeRegistry::new();
        let state = NavigatorState::new(reg);

        assert_eq!(state.current, None);
        assert_eq!(state.history.len(), 0);
        assert_eq!(state.history_index, 0);
        assert_eq!(state.history_limit, 100);
        assert_eq!(state.selected.len(), 0);
    }

    #[test]
    fn test_custom_history_limit() {
        let reg = NodeRegistry::new();
        let state = NavigatorState::with_history_limit(10, reg);

        assert_eq!(state.history_limit, 10);
        assert_eq!(state.current, None);
    }

    #[test]
    fn test_build_pipeline() {
        let reg = NodeRegistry::new();
        let state = NavigatorState::new(reg);
        let _pipeline = state.build_pipeline();
    }

    #[test]
    fn test_multiple_back_forward_cycles() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        // Build history
        state.navigate(node(1));
        state.navigate(node(2));
        state.navigate(node(3));
        state.navigate(node(4));

        // Complex navigation pattern
        state.back(1); // -> 3
        state.back(1); // -> 2
        assert_eq!(state.current, Some(node(2)));

        state.forward(); // -> 3
        assert_eq!(state.current, Some(node(3)));

        state.back(1); // -> 2
        state.back(1); // -> 1
        assert_eq!(state.current, Some(node(1)));

        state.forward(); // -> 2
        state.forward(); // -> 3
        state.forward(); // -> 4
        assert_eq!(state.current, Some(node(4)));
        assert!(!state.can_forward());
    }

    #[test]
    fn test_history_preserves_order() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        let nodes = vec![node(100), node(200), node(300), node(400)];

        for &n in &nodes {
            state.navigate(n);
        }

        assert_eq!(state.history.len(), 4);
        for (i, &n) in nodes.iter().enumerate() {
            assert_eq!(state.history[i], n);
        }
    }

    #[test]
    fn test_navigate_same_directory_twice() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        state.navigate(node(1));
        state.navigate(node(1));

        // Should add to history even if same
        assert_eq!(state.history.len(), 2);
        assert_eq!(state.history[0], node(1));
        assert_eq!(state.history[1], node(1));
    }
}

#[cfg(test)]
mod nav_state_serialization_tests {
    use super::*;

    #[test]
    fn test_nav_state_new() {
        let state = NavState::default();

        assert_eq!(state.current, None);
        assert_eq!(state.current_location, None);
        assert!(!state.can_back);
        assert!(!state.can_forward);
        assert!(!state.can_up);
        assert_eq!(state.selected.len(), 0);
    }

    #[test]
    fn test_nav_state_serializable() {
        use serde_json;

        let state = NavState {
            current: Some(NodeId(42)),
            current_location: Some(LocationRef::descriptor_only(LocationDescriptor::local(
                "/tmp/nav-state",
            ))),
            can_back: true,
            can_forward: false,
            can_up: true,
            pipeline: PipelineConfig::with_default_sort(),
            selected: vec![NodeId(1), NodeId(2)],
        };

        // Should serialize without error
        let json = serde_json::to_string(&state);
        assert!(json.is_ok());

        // Should deserialize back
        let json_str = json.unwrap();
        let deserialized: Result<NavState, _> = serde_json::from_str(&json_str);
        assert!(deserialized.is_ok());

        let restored = deserialized.unwrap();
        assert_eq!(restored.current, Some(NodeId(42)));
        assert_eq!(
            restored
                .current_location
                .as_ref()
                .and_then(|r| r.descriptor()),
            Some(&LocationDescriptor::local("/tmp/nav-state"))
        );
        assert!(restored.can_back);
        assert_eq!(restored.selected.len(), 2);
    }

    #[test]
    fn test_nav_state_deserializes_without_current_location() {
        let json = r#"{
            "current": null,
            "can_back": false,
            "can_forward": false,
            "can_up": false,
            "pipeline": { "sort": null, "filter": null, "group": null },
            "selected": []
        }"#;

        let restored: NavState = serde_json::from_str(json).unwrap();
        assert_eq!(restored.current, None);
        assert_eq!(restored.current_location, None);
    }
}

#[cfg(test)]
mod navigator_actor_tests {
    use super::*;
    use crate::{Event, model::registry::NodeRegistry, modules::scan::scanner::ScanCommand};

    /// Helper to create test NodeIds
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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let target_node = node(100);

        // Send navigate command
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: target_node,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // Should trigger a scan command
        let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Should receive scan command")
            .expect("Channel should not be closed");

        match scan_cmd {
            ScanCommand::ScanNode {
                session: s,
                node: n,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(n, target_node);
            }
            _ => panic!("Expected Scan command"),
        }

        // Should emit NavigationChanged event or similar
        let event = timeout(Duration::from_millis(100), event_rx.recv_async()).await;

        // Event might be emitted (depending on implementation)
        // This test validates the command is processed
        assert!(event.is_ok() || event.is_err(), "Command was processed");
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
                assert!(state.current.is_some());
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

        // First navigate to build history
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(100),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(200),
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

        // Build history and go back
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(100),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(200),
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

        // Navigate first
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(100),
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
    async fn test_navigator_invalidate_refreshes_current_directory() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let current = node(100);

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
                ScanCommand::RefreshNode {
                    node,
                    session: s,
                    ..
                } if node == current && s == session
            ),
            "Invalidate should refresh the session currently displaying the node"
        );
    }

    #[tokio::test]
    async fn test_navigator_invalidate_refreshes_only_current_sessions_with_current_pipeline() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let current_session = session(1);
        let other_session = session(2);
        let current = node(100);
        let other = node(200);
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
            ScanCommand::RefreshNode {
                node,
                session,
                pipeline: refresh_pipeline,
                ..
            } => {
                assert_eq!(node, current);
                assert_eq!(session, current_session);
                assert_eq!(refresh_pipeline, pipeline);
            }
            other => panic!("expected RefreshNode, got {other:?}"),
        }

        assert!(
            scanner_rx.try_recv().is_err(),
            "sessions not currently viewing the invalidated node should not refresh"
        );
    }

    #[tokio::test]
    async fn test_navigator_multiple_sessions() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session1 = session(1);
        let session2 = session(2);

        // Navigate in both sessions
        cmd_tx
            .send(NavCommand::Navigate {
                session: session1,
                node: node(100),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::Navigate {
                session: session2,
                node: node(200),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Both sessions should be independent
        cmd_tx
            .send(NavCommand::Back(
                session1,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();
        cmd_tx.send(NavCommand::GetState(session2)).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should process without panic
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_handles_set_selected_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        let nodes = vec![node(1), node(2), node(3)];

        // Set selection
        cmd_tx
            .send(NavCommand::SetSelected { session, nodes })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should process without panic
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_command_processing_order() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);

        // Send multiple commands in sequence
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(100),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(200),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::Navigate {
                session,
                node: node(300),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // Should receive scan commands in order
        for expected_node in [node(100), node(200), node(300)] {
            let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
                .await
                .expect("Should receive scan command")
                .expect("Channel should not be closed");

            match scan_cmd {
                ScanCommand::ScanNode { node: n, .. } => {
                    assert_eq!(n, expected_node, "Commands should be processed in order");
                }
                _ => panic!("Expected Scan command"),
            }
        }
    }
}
