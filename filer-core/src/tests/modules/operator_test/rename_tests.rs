#[cfg(test)]
mod rename_tests {
    use super::*;

    #[tokio::test]
    async fn test_rename_file() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/old_name.txt");
        let src_id = register(&registry, &src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Rename {
                source: src_id,
                new_name: "new_name.txt".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationCompleteCompat {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Rename));
                assert!(success);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        let renames = provider.get_rename_calls();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].0, src_path);
        assert_eq!(
            renames[0].1,
            PathBuf::from("/home/user/new_name.txt"),
            "New path should be parent + new_name"
        );
    }

    #[tokio::test]
    async fn test_rename_directory() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/old_dir");
        let src_id = register(&registry, &src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Rename {
                source: src_id,
                new_name: "new_dir".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationCompleteCompat {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Rename));
                assert!(success);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        let renames = provider.get_rename_calls();
        assert_eq!(renames[0].1, PathBuf::from("/home/user/new_dir"));
    }

    #[tokio::test]
    async fn test_rename_collision_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/file_a.txt");
        let src_id = register(&registry, &src_path);

        let collision_path = PathBuf::from("/home/user/file_b.txt");
        provider.add_existing(&collision_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Rename {
                source: src_id,
                new_name: "file_b.txt".to_string(),
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
                source: ErrorTarget::Path(src_path),
                destination: ErrorTarget::Path(collision_path),
            })
        );

        assert!(
            provider.get_rename_calls().is_empty(),
            "Should not rename when destination exists"
        );
    }
}
