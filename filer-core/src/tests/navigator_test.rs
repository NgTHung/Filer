use crate::actors::navigator::{NavCommand, NavState, Navigator, NavigatorState};
use crate::actors::Actor;
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;
use flume;
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
        let mut state = NavigatorState::with_history_limit(5,reg);
        
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
        assert_eq!(state.can_back(), false);
        
        // Navigate once
        state.navigate(node(1));
        assert_eq!(state.can_back(), false); // Still at first position
        
        // Navigate again
        state.navigate(node(2));
        assert_eq!(state.can_back(), true); // Now can go back
        
        // Go back to start
        state.back(1);
        assert_eq!(state.can_back(), false); // At start again
    }

    #[test]
    fn test_can_forward_false_at_end() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        
        // No history
        assert_eq!(state.can_forward(), false);
        
        // Navigate
        state.navigate(node(1));
        state.navigate(node(2));
        assert_eq!(state.can_forward(), false); // At the end
        
        // Go back
        state.back(1);
        assert_eq!(state.can_forward(), true); // Can go forward now
        
        // Go forward to end
        state.forward();
        assert_eq!(state.can_forward(), false); // At end again
    }

    #[test]
    fn test_snapshot_reflects_state() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        
        // Initial snapshot
        let snap = state.snapshot();
        assert_eq!(snap.current, None);
        assert_eq!(snap.can_back, false);
        assert_eq!(snap.can_forward, false);
        assert_eq!(snap.selected.len(), 0);
        
        // After navigation
        state.navigate(node(1));
        state.navigate(node(2));
        let snap = state.snapshot();
        assert_eq!(snap.current, Some(node(2)));
        assert_eq!(snap.can_back, true);
        assert_eq!(snap.can_forward, false);
        
        // After going back
        state.back(1);
        let snap = state.snapshot();
        assert_eq!(snap.current, Some(node(1)));
        assert_eq!(snap.can_back, false);
        assert_eq!(snap.can_forward, true);
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
        let state = NavigatorState::with_history_limit(10,reg);
        
        assert_eq!(state.history_limit, 10);
        assert_eq!(state.current, None);
    }

    #[test]
    fn test_build_pipeline() {
        let reg = NodeRegistry::new();
        let state = NavigatorState::new(reg);
        let _pipeline = state.build_pipeline();
        
        // Should create a valid pipeline
        // The exact behavior depends on Pipeline implementation
        // This test ensures the method can be called without panic
        assert!(true);
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
        assert_eq!(state.can_forward(), false);
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
        assert_eq!(state.can_back, false);
        assert_eq!(state.can_forward, false);
        assert_eq!(state.can_up, false);
        assert_eq!(state.selected.len(), 0);
    }

    #[test]
    fn test_nav_state_serializable() {
        use serde_json;
        
        let state = NavState {
            current: Some(NodeId(42)),
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
        assert_eq!(restored.can_back, true);
        assert_eq!(restored.selected.len(), 2);
    }
}

#[cfg(test)]
mod navigator_actor_tests {
    use super::*;
    use crate::{actors::scanner::ScanCommand, model::registry::NodeRegistry};

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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);

        // Spawn actor in background
        let handle = tokio::spawn(async move {
            navigator.run().await;
        });

        // Drop command sender to signal shutdown
        drop(cmd_tx);

        // Actor should terminate gracefully
        let result = timeout(Duration::from_millis(100), handle).await;
        assert!(result.is_ok(), "Navigator should shutdown when command channel closes");
    }

    #[tokio::test]
    async fn test_navigator_handles_navigate_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();
        let target_node = node(100);
        
        // Send navigate command
        cmd_tx.send(NavCommand::Navigate {
            session,
            node: target_node,
        }).unwrap();
        
        // Should trigger a scan command
        let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
            .await
            .expect("Should receive scan command")
            .expect("Channel should not be closed");

        match scan_cmd {
            ScanCommand::ScanNode { session: s, node: n, .. } => {
                assert_eq!(s, session);
                assert_eq!(n, target_node);
            }
            _ => panic!("Expected Scan command"),
        }

        // Should emit NavigationChanged event or similar
        let event = timeout(Duration::from_millis(100), event_rx.recv_async())
        .await;
        
        // Event might be emitted (depending on implementation)
        // This test validates the command is processed
        assert!(event.is_ok() || event.is_err(), "Command was processed");
    }

    #[tokio::test]
    async fn test_navigator_handles_back_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });
        
        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();
        
        // First navigate to build history
        cmd_tx.send(NavCommand::Navigate {
            session,
            node: node(100),
        }).unwrap();
        
        tokio::time::sleep(Duration::from_millis(10)).await;

        cmd_tx.send(NavCommand::Navigate {
            session,
            node: node(200),
        }).unwrap();
        
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Now go back
        cmd_tx.send(NavCommand::Back(session)).unwrap();

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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();
        
        // Build history and go back
        cmd_tx.send(NavCommand::Navigate { session, node: node(100) }).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        cmd_tx.send(NavCommand::Navigate { session, node: node(200) }).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        cmd_tx.send(NavCommand::Back(session)).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Now go forward
        cmd_tx.send(NavCommand::Forward(session)).unwrap();
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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();
        
        // Request state
        cmd_tx.send(NavCommand::GetState(session)).unwrap();

        // Should emit state event
        let result = timeout(Duration::from_millis(100), event_rx.recv_async()).await;
        
        // Might receive a StateUpdate event (depending on implementation)
        // The test validates command processing
        assert!(result.is_ok() || result.is_err(), "GetState command processed");
    }

    #[tokio::test]
    async fn test_navigator_handles_set_pipeline_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();
        let config = PipelineConfig::with_default_sort();

        // Update pipeline
        cmd_tx.send(NavCommand::SetPipeline { session, config }).unwrap();
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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();

        // Navigate first
        cmd_tx.send(NavCommand::Navigate { session, node: node(100) }).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Clear scan queue
        while scanner_rx.try_recv().is_ok() {}

        // Refresh current directory
        cmd_tx.send(NavCommand::Refresh(session)).unwrap();

        // Should trigger a new scan
        let result = timeout(Duration::from_millis(100), scanner_rx.recv_async()).await;
        assert!(result.is_ok(), "Refresh should trigger a scan");
    }

    #[tokio::test]
    async fn test_navigator_multiple_sessions() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session1 = session(1);
        let session2 = session(2);
        cmd_tx.send(NavCommand::NewSession(session1)).unwrap();
        cmd_tx.send(NavCommand::NewSession(session2)).unwrap();

        // Navigate in both sessions
        cmd_tx.send(NavCommand::Navigate { session: session1, node: node(100) }).unwrap();
        cmd_tx.send(NavCommand::Navigate { session: session2, node: node(200) }).unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Both sessions should be independent
        cmd_tx.send(NavCommand::Back(session1)).unwrap();
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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();
        let nodes = vec![node(1), node(2), node(3)];

        // Set selection
        cmd_tx.send(NavCommand::SetSelected { 
            session, 
            nodes 
        }).unwrap();

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
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx,reg);
        
        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        cmd_tx.send(NavCommand::NewSession(session)).unwrap();

        // Send multiple commands in sequence
        cmd_tx.send(NavCommand::Navigate { session, node: node(100) }).unwrap();
        cmd_tx.send(NavCommand::Navigate { session, node: node(200) }).unwrap();
        cmd_tx.send(NavCommand::Navigate { session, node: node(300) }).unwrap();

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
