#[cfg(test)]
mod navigator_state_tests {
    use crate::model::registry::NodeRegistry;

    use super::*;

    fn location(id: u64) -> LocationRef {
        LocationRef::descriptor_only(LocationDescriptor::local(format!("/tmp/navigator-{id}")))
    }

    #[test]
    fn test_navigate_updates_current() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = location(1);
        let second = location(2);

        assert_eq!(state.current_location(), None);

        state.navigate_location(first.clone(), None);
        assert_eq!(state.current_location(), Some(&first));

        state.navigate_location(second.clone(), None);
        assert_eq!(state.current_location(), Some(&second));
    }

    #[test]
    fn test_navigate_adds_to_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = location(1);
        let second = location(2);
        let third = location(3);

        state.navigate_location(first.clone(), None);
        state.navigate_location(second.clone(), None);
        state.navigate_location(third.clone(), None);

        assert_eq!(state.history.len(), 3);
        assert_eq!(state.current_location(), Some(&third));
        assert_eq!(state.back(1), None);
        assert_eq!(state.current_location(), Some(&second));
        assert_eq!(state.back(1), None);
        assert_eq!(state.current_location(), Some(&first));
    }

    #[test]
    fn test_back_moves_history_index() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = location(1);
        let second = location(2);
        let third = location(3);

        state.navigate_location(first.clone(), None);
        state.navigate_location(second, None);
        state.navigate_location(third, None);

        assert_eq!(state.history_index, 0);
        assert_eq!(state.current_location(), Some(&location(3)));

        assert_eq!(state.back(1), None);
        assert_eq!(state.history_index, 1);
        assert_eq!(state.current_location(), Some(&location(2)));

        assert_eq!(state.back(1), None);
        assert_eq!(state.history_index, 2);
        assert_eq!(state.current_location(), Some(&first));

        assert_eq!(state.back(1), None);
        assert_eq!(state.history_index, 2);
    }

    #[test]
    fn test_forward_moves_history_index() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        state.navigate_location(location(1), None);
        state.navigate_location(location(2), None);
        state.navigate_location(location(3), None);

        state.back(1);
        state.back(1);
        assert_eq!(state.current_location(), Some(&location(1)));
        assert_eq!(state.history_index, 2);

        assert_eq!(state.forward(), None);
        assert_eq!(state.history_index, 1);
        assert_eq!(state.current_location(), Some(&location(2)));

        assert_eq!(state.forward(), None);
        assert_eq!(state.history_index, 0);
        assert_eq!(state.current_location(), Some(&location(3)));

        assert_eq!(state.forward(), None);
        assert_eq!(state.history_index, 0);
    }

    #[test]
    fn test_navigate_up() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        state.navigate_location(location(100), None);
        state.navigate_location(location(200), None);

        assert_eq!(state.current_location(), Some(&location(200)));
        assert_eq!(state.history.len(), 2);
    }

    #[test]
    fn test_history_limit_enforced() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::with_history_limit(5, reg);

        for id in 1..=10 {
            state.navigate_location(location(id), None);
        }

        assert_eq!(state.history.len(), 5);
        assert_eq!(state.current_location(), Some(&location(10)));

        for id in (6..=9).rev() {
            assert_eq!(state.back(1), None);
            assert_eq!(state.current_location(), Some(&location(id)));
        }
        assert_eq!(state.back(1), None);
        assert_eq!(state.current_location(), Some(&location(6)));
        assert_eq!(state.history_index, 4);
    }

    #[test]
    fn test_navigate_clears_forward_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        state.navigate_location(location(1), None);
        state.navigate_location(location(2), None);
        state.navigate_location(location(3), None);

        state.back(1);
        assert_eq!(state.current_location(), Some(&location(2)));
        assert_eq!(state.history.len(), 3);

        state.navigate_location(location(4), None);
        assert_eq!(state.history.len(), 3);
        assert_eq!(state.current_location(), Some(&location(4)));
        assert_eq!(state.forward(), None);
    }

    #[test]
    fn test_can_back_false_at_start() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        assert!(!state.can_back());

        state.navigate_location(location(1), None);
        assert!(!state.can_back());

        state.navigate_location(location(2), None);
        assert!(state.can_back());

        state.back(1);
        assert!(!state.can_back());
    }

    #[test]
    fn test_can_forward_false_at_end() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        assert!(!state.can_forward());

        state.navigate_location(location(1), None);
        state.navigate_location(location(2), None);
        assert!(!state.can_forward());

        state.back(1);
        assert!(state.can_forward());

        state.forward();
        assert!(!state.can_forward());
    }

    #[test]
    fn test_snapshot_reflects_state() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        let snap = state.snapshot();
        assert_eq!(snap.current, None);
        assert_eq!(snap.current_location, None);
        assert!(!snap.can_back);
        assert!(!snap.can_forward);
        assert_eq!(snap.selected.len(), 0);

        state.navigate_location(location(1), None);
        state.navigate_location(location(2), None);
        let snap = state.snapshot();
        assert_eq!(snap.current, None);
        assert_eq!(snap.current_location, Some(location(2)));
        assert!(snap.can_back);
        assert!(!snap.can_forward);

        state.back(1);
        let snap = state.snapshot();
        assert_eq!(snap.current, None);
        assert_eq!(snap.current_location, Some(location(1)));
        assert!(!snap.can_back);
        assert!(snap.can_forward);
    }

    #[test]
    // Compatibility pin for API-006: navigate_with_location still carries NodeId identity.
    fn test_compat_navigate_with_location_updates_both_identities() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let location = location(10);
        let node = NodeId(10);

        state.navigate_with_location(node, Some(location.clone()));

        assert_eq!(state.current_compat_node(), Some(node));
        assert_eq!(state.current_location(), Some(&location));
    }

    #[test]
    fn test_location_navigation_stores_provider_location() {
        let reg = NodeRegistry::new();
        let path = std::path::PathBuf::from("/tmp/location-node");
        let location = LocationRef::from_location(&Location::local(path.clone()));
        let mut state = NavigatorState::new(reg);

        state.navigate_location(location.clone(), None);

        assert_eq!(state.current_location(), Some(&location));
    }

    #[test]
    fn test_back_and_forward_restore_current_location() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = location(1);
        let second = location(2);

        state.navigate_location(first.clone(), None);
        state.navigate_location(second.clone(), None);

        assert_eq!(state.back(1), None);
        assert_eq!(state.current_location(), Some(&first));

        assert_eq!(state.forward(), None);
        assert_eq!(state.current_location(), Some(&second));
    }

    #[test]
    fn test_navigate_after_back_clears_forward_location_history() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let first = location(1);
        let second = location(2);
        let replacement = location(3);

        state.navigate_location(first, None);
        state.navigate_location(second, None);
        state.back(1);
        state.navigate_location(replacement.clone(), None);

        assert_eq!(state.history.len(), 2);
        assert_eq!(state.current_location(), Some(&replacement));
        assert_eq!(state.forward(), None);
    }

    #[test]
    fn test_location_navigation_can_store_without_node_identity() {
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
    fn test_location_history_restores_without_node_identity() {
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
        assert_eq!(state.current_location(), None);
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
        assert_eq!(state.current_location(), None);
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

        for id in 1..=4 {
            state.navigate_location(location(id), None);
        }

        state.back(1);
        state.back(1);
        assert_eq!(state.current_location(), Some(&location(2)));

        state.forward();
        assert_eq!(state.current_location(), Some(&location(3)));

        state.back(1);
        state.back(1);
        assert_eq!(state.current_location(), Some(&location(1)));

        state.forward();
        state.forward();
        state.forward();
        assert_eq!(state.current_location(), Some(&location(4)));
        assert!(!state.can_forward());
    }

    #[test]
    fn test_history_preserves_order() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);

        for id in 1..=4 {
            state.navigate_location(location(id), None);
        }

        for id in (1..=4).rev() {
            assert_eq!(state.current_location(), Some(&location(id)));
            if id > 1 {
                state.back(1);
            }
        }
    }

    #[test]
    fn test_navigate_same_directory_twice() {
        let reg = NodeRegistry::new();
        let mut state = NavigatorState::new(reg);
        let location = location(1);

        state.navigate_location(location.clone(), None);
        state.navigate_location(location.clone(), None);

        assert_eq!(state.history.len(), 2);
        assert_eq!(state.current_location(), Some(&location));
        state.back(1);
        assert_eq!(state.current_location(), Some(&location));
    }
}
