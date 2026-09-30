#[cfg(test)]
mod location_operation_tests {
    use super::*;

    fn local_ref(path: &str) -> LocationRef {
        LocationRef::from_location(&Location::local(path))
    }

    fn assert_location_completion(
        event: Event,
        expected_operation: OperationId,
        expected_kind: OperationKind,
        expected_session: SessionId,
        expected_affected: Vec<LocationRef>,
    ) {
        match event {
            Event::OperationComplete {
                operation_id,
                operation,
                success,
                affected,
                session,
            } => {
                assert_eq!(operation_id, expected_operation);
                assert_eq!(operation, expected_kind);
                assert!(success);
                assert_eq!(session, expected_session);
                assert_eq!(affected, expected_affected);
            }
            other => panic!("Expected OperationComplete, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn copy_location_emits_affected_location() {
        let provider = MockOpsProvider::new();
        provider.add_metadata("/home/user/source.txt", MockOpsProvider::make_file("source.txt", "/home/user", 10));
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref("/home/user/source.txt")],
                destination: local_ref("/home/user/dest"),
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_location_completion(
            final_event,
            operation,
            OperationKind::Copy,
            session,
            vec![local_ref("/home/user/dest/source.txt")],
        );
    }

    #[tokio::test]
    async fn move_location_emits_affected_location() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref("/home/user/source.txt")],
                destination: local_ref("/home/user/dest"),
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_location_completion(
            final_event,
            operation,
            OperationKind::Move,
            session,
            vec![local_ref("/home/user/dest/source.txt")],
        );
    }

    #[tokio::test]
    async fn delete_location_emits_affected_location() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let target = local_ref("/home/user/old.txt");
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![target.clone()],
                trash: false,
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_location_completion(
            final_event,
            operation,
            OperationKind::Delete,
            session,
            vec![target],
        );
    }

    #[tokio::test]
    async fn rename_location_emits_affected_location() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Rename {
                source: local_ref("/home/user/old.txt"),
                new_name: "new.txt".to_string(),
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_location_completion(
            final_event,
            operation,
            OperationKind::Rename,
            session,
            vec![local_ref("/home/user/new.txt")],
        );
    }

    #[tokio::test]
    async fn create_folder_location_emits_affected_location() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::CreateFolder {
                parent: local_ref("/home/user"),
                name: "created".to_string(),
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_location_completion(
            final_event,
            operation,
            OperationKind::CreateFolder,
            session,
            vec![local_ref("/home/user/created")],
        );
    }

    #[tokio::test]
    async fn test_create_file_location_emits_location_completion() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let parent = local_ref("/home/user");
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request = RequestId::new();
        let operation = OperationId::new();

        cmd_tx
            .send(OpsCommand::CreateFile {
                parent,
                name: "new_file.txt".to_string(),
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_location_completion(
            final_event,
            operation,
            OperationKind::CreateFile,
            session,
            vec![local_ref("/home/user/new_file.txt")],
        );

        let writes = provider.get_write_calls();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, PathBuf::from("/home/user/new_file.txt"));
    }

    #[tokio::test]
    async fn test_delete_location_segmented_route_emits_operation_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let segmented = LocationRef::Descriptor(
            LocationDescriptor::local("/home/user/archive.zip").with_segment(
                LocationSegment::ArchiveMember {
                    path: PathBuf::from("inner.txt"),
                },
            ),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![segmented],
                trash: false,
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request, operation);
        assert!(
            provider.get_delete_calls().is_empty(),
            "segmented locations must not reach provider delete"
        );
    }

    #[tokio::test]
    async fn unsupported_location_write_exposes_provider_capability_context() {
        let provider = MockOpsProvider::new();
        provider.set_write_supported(false);
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let parent = LocationRef::from_location(&Location::local("/home/user"));
        let expected_location = parent.clone();
        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);

        cmd_tx
            .send(OpsCommand::CreateFile {
                parent,
                name: "blocked.txt".to_string(),
                event_mode: OperationEventMode::Location,
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request, operation);
        match final_event {
            Event::Error {
                code: ErrorCode::ProviderCapabilityUnavailable,
                context: Some(context),
                ..
            } => assert!(matches!(
                *context,
                ErrorContext::ProviderCapability {
                    provider: ProviderRef::Local,
                    location,
                    capability: LocationCapabilityError::WriteUnsupported,
                } if location == expected_location
            )),
            other => panic!("Expected provider capability error, got {other:?}"),
        }
    }
}
