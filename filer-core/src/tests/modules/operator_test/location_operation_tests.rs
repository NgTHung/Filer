#[cfg(test)]
mod location_operation_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_file_location_emits_location_completion() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let parent = LocationRef::from_location(&Location::local("/home/user"));
        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request = RequestId::new();
        let operation = OperationId::new();

        cmd_tx
            .send(OpsCommand::CreateFileLocation {
                parent,
                name: "new_file.txt".to_string(),
                session,
                request,
                operation,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation_id,
                operation: kind,
                success,
                affected,
                session: s,
            } => {
                assert_eq!(operation_id, operation);
                assert!(matches!(kind, OperationKind::CreateFile));
                assert!(success);
                assert_eq!(s, session);
                assert_eq!(affected.len(), 1);
                assert!(matches!(affected[0], LocationRef::Full { .. }));
            }
            other => panic!("Expected OperationComplete, got {other:?}"),
        }

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
            .send(OpsCommand::DeleteLocation {
                targets: vec![segmented],
                trash: false,
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
            .send(OpsCommand::CreateFileLocation {
                parent,
                name: "blocked.txt".to_string(),
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
