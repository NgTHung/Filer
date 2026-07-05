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
        assert_eq!(state.current, Some(node(3)));
        assert_eq!(state.current_location, Some(replacement));
        assert_eq!(state.forward(), None);
    }

    #[test]
    fn test_location_navigation_can_store_without_compat_node() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let location = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-authority.zip").archive_member("src"),
        );

        state.navigate_location(location.clone(), None);

        let snapshot = state.snapshot();
        assert_eq!(state.current, None);
        assert_eq!(snapshot.current, None);
        assert_eq!(state.current_location(), Some(&location));
        assert_eq!(snapshot.current_location, Some(location));
    }

    #[test]
    fn test_location_history_restores_without_compat_nodes() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-history.zip").archive_member("first"),
        );
        let second = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-history.zip").archive_member("second"),
        );

        state.navigate_location(first.clone(), None);
        state.navigate_location(second.clone(), None);

        assert_eq!(state.back(1), None);
        assert_eq!(state.current_location(), Some(&first));

        assert_eq!(state.forward(), None);
        assert_eq!(state.current_location(), Some(&second));
    }

    #[test]
    fn test_location_navigation_after_back_clears_forward_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-clear.zip").archive_member("first"),
        );
        let second = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-clear.zip").archive_member("second"),
        );
        let replacement = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-clear.zip").archive_member("replacement"),
        );

        state.navigate_location(first, None);
        state.navigate_location(second, None);
        state.back(1);
        state.navigate_location(replacement.clone(), None);

        assert_eq!(state.history.len(), 2);
        assert_eq!(state.current, None);
        assert_eq!(state.current_location(), Some(&replacement));
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
