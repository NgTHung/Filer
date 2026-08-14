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

        // API-007 pin: NavState still serializes internal NodeId handles.
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
        assert_eq!(restored.selected, vec![NodeId(1), NodeId(2)]);
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
