#[cfg(test)]
mod nav_state_serialization_tests {
    use super::*;

    #[test]
    fn test_nav_state_new() {
        let state = NavState::default();

        assert_eq!(state.current, None);
        assert!(!state.can_back);
        assert!(!state.can_forward);
        assert!(!state.can_up);
        assert!(state.selected.is_empty());
    }

    #[test]
    fn test_nav_state_serializable_with_location_identity() {
        let current = Location::local("/tmp/nav-state");
        let selected = vec![
            LocationRef::from_location(&Location::local("/tmp/one")),
            LocationRef::from_location(&Location::local("/tmp/two")),
        ];
        let state = NavState {
            current: Some(LocationRef::from_location(&current)),
            can_back: true,
            can_forward: false,
            can_up: true,
            pipeline: PipelineConfig::with_default_sort(),
            selected: selected.clone(),
        };

        let json = serde_json::to_string(&state).expect("location state should serialize");
        let restored: NavState = serde_json::from_str(&json).expect("location state should decode");

        assert_eq!(restored.current, state.current);
        assert!(restored.can_back);
        assert_eq!(restored.selected, selected);
        assert!(!json.contains("current_location"));
    }

    #[test]
    fn test_nav_state_rejects_node_id_shaped_current() {
        let json = r#"{
            "current": 42,
            "can_back": false,
            "can_forward": false,
            "can_up": false,
            "pipeline": { "sort": null, "filter": null, "group": null },
            "selected": []
        }"#;

        assert!(serde_json::from_str::<NavState>(json).is_err());
    }

    #[test]
    fn test_nav_state_defaults_missing_current() {
        let json = r#"{
            "can_back": false,
            "can_forward": false,
            "can_up": false,
            "pipeline": { "sort": null, "filter": null, "group": null },
            "selected": []
        }"#;

        let restored: NavState = serde_json::from_str(json).expect("missing current is optional");
        assert_eq!(restored.current, None);
    }
}
