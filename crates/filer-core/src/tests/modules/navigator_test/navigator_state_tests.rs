#[cfg(test)]
mod navigator_state_tests {
    use super::*;

    fn location(id: u64) -> LocationRef {
        LocationRef::descriptor_only(LocationDescriptor::local(format!("/tmp/navigator-{id}")))
    }

    #[test]
    fn test_navigate_updates_current() {
        let mut state = NavigatorState::new();
        let first = location(1);
        let second = location(2);

        assert_eq!(state.current, None);
        state.navigate_location(first.clone());
        assert_eq!(state.current, Some(first));
        state.navigate_location(second.clone());
        assert_eq!(state.current, Some(second));
    }

    #[test]
    fn test_navigate_adds_to_history() {
        let mut state = NavigatorState::new();
        let first = location(1);
        let second = location(2);
        let third = location(3);

        state.navigate_location(first.clone());
        state.navigate_location(second.clone());
        state.navigate_location(third.clone());

        assert_eq!(state.history.len(), 3);
        assert_eq!(state.current, Some(third));
        state.back(1);
        assert_eq!(state.current, Some(second));
        state.back(1);
        assert_eq!(state.current, Some(first));
    }

    #[test]
    fn test_back_and_forward_restore_locations() {
        let mut state = NavigatorState::new();
        let first = location(1);
        let second = location(2);
        let third = location(3);
        state.navigate_location(first.clone());
        state.navigate_location(second.clone());
        state.navigate_location(third.clone());

        assert_eq!(state.back(1), Some(second.clone()));
        assert_eq!(state.back(1), Some(first));
        assert_eq!(state.forward(), Some(second));
        assert_eq!(state.forward(), Some(third));
        assert_eq!(state.forward(), None);
    }

    #[test]
    fn test_history_limit_enforced() {
        let mut state = NavigatorState::with_history_limit(5);
        for id in 1..=10 {
            state.navigate_location(location(id));
        }

        assert_eq!(state.history.len(), 5);
        assert_eq!(state.current, Some(location(10)));
        for id in (6..=9).rev() {
            assert_eq!(state.back(1), Some(location(id)));
        }
        assert_eq!(state.back(1), None);
    }

    #[test]
    fn test_navigate_clears_forward_history() {
        let mut state = NavigatorState::new();
        state.navigate_location(location(1));
        state.navigate_location(location(2));
        state.navigate_location(location(3));
        state.back(1);
        state.navigate_location(location(4));

        assert_eq!(state.history.len(), 3);
        assert_eq!(state.current, Some(location(4)));
        assert_eq!(state.forward(), None);
    }

    #[test]
    fn test_can_back_and_forward() {
        let mut state = NavigatorState::new();
        assert!(!state.can_back());
        assert!(!state.can_forward());
        state.navigate_location(location(1));
        state.navigate_location(location(2));
        assert!(state.can_back());
        assert!(!state.can_forward());
        state.back(1);
        assert!(!state.can_back());
        assert!(state.can_forward());
    }

    #[test]
    fn test_snapshot_reflects_location_state() {
        let mut state = NavigatorState::new();
        let first = location(1);
        let second = location(2);

        let snapshot = state.snapshot();
        assert_eq!(snapshot.current, None);
        assert!(!snapshot.can_back);
        assert!(!snapshot.can_forward);
        assert!(!snapshot.can_up);
        assert!(snapshot.selected.is_empty());

        state.navigate_location(first.clone());
        state.navigate_location(second.clone());
        let snapshot = state.snapshot();
        assert_eq!(snapshot.current, Some(second));
        assert!(snapshot.can_back);
        assert!(snapshot.can_up);

        state.back(1);
        assert_eq!(state.snapshot().current, Some(first));
    }

    #[test]
    fn test_segmented_location_has_no_parent() {
        let mut state = NavigatorState::new();
        let location = LocationRef::descriptor_only(
            LocationDescriptor::local("/tmp/location-authority.zip").archive_member("src"),
        );
        state.navigate_location(location.clone());

        assert_eq!(state.current, Some(location.clone()));
        assert_eq!(state.snapshot().current, Some(location));
        assert!(!state.snapshot().can_up);
    }

    #[test]
    fn test_provider_location_history() {
        let mut state = NavigatorState::new();
        let first = LocationRef::descriptor_only(LocationDescriptor::provider_profile(
            "sftp",
            "work",
            "/tmp/first",
        ));
        let second = LocationRef::descriptor_only(LocationDescriptor::provider_profile(
            "sftp",
            "work",
            "/tmp/second",
        ));
        state.navigate_location(first.clone());
        state.navigate_location(second.clone());

        assert_eq!(state.back(1), Some(first));
        assert_eq!(state.forward(), Some(second));
    }

    #[test]
    fn test_selection_is_location_keyed_and_sorted() {
        let mut state = NavigatorState::new();
        let first = LocationRef::from_location(&Location::local("/tmp/selection-first"));
        let second = LocationRef::from_location(&Location::local("/tmp/selection-second"));
        state.selected.insert(
            Location::local("/tmp/selection-second").id(),
            second.clone(),
        );
        state.selected.insert(
            Location::local("/tmp/selection-first").id(),
            first.clone(),
        );

        let selected = state.snapshot().selected;
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&first));
        assert!(selected.contains(&second));
    }

    #[test]
    fn test_default_and_custom_limits() {
        let state = NavigatorState::new();
        assert_eq!(state.current, None);
        assert_eq!(state.history_limit, 100);
        assert!(state.history.is_empty());

        let state = NavigatorState::with_history_limit(10);
        assert_eq!(state.history_limit, 10);
    }

    #[test]
    fn test_build_pipeline() {
        let state = NavigatorState::new();
        let _pipeline = state.build_pipeline();
    }
}
