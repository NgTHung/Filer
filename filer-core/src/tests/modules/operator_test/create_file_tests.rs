#[cfg(test)]
mod create_file_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_file() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let parent_path = PathBuf::from("/home/user");
        let parent_id = register(&registry, &parent_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::CreateFile {
                parent: parent_id,
                name: "new_file.txt".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationCompleteCompat {
                operation,
                success,
                affected,
                session: s,
                ..
            } => {
                assert!(matches!(operation, OperationKind::CreateFile));
                assert!(success);
                assert!(!affected.is_empty());
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        let writes = provider.get_write_calls();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, PathBuf::from("/home/user/new_file.txt"));
        assert!(writes[0].1.is_empty(), "New file should be created empty");
    }

    #[tokio::test]
    async fn test_create_file_collision_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let parent_path = PathBuf::from("/home/user");
        let parent_id = register(&registry, &parent_path);

        provider.add_existing(PathBuf::from("/home/user/exists.txt"));

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::CreateFile {
                parent: parent_id,
                name: "exists.txt".to_string(),
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        assert_error_correlation(&final_event, session, request_id, operation_id);
        assert_eq!(
            error_context(&final_event),
            Some(&ErrorContext::Collision {
                source: ErrorTarget::Path(parent_path),
                destination: ErrorTarget::Path(PathBuf::from("/home/user/exists.txt")),
            })
        );

        assert!(
            provider.get_write_calls().is_empty(),
            "Should not write when file already exists"
        );
    }
}
