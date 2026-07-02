#[cfg(test)]
mod delete_tests {
    use super::*;

    #[tokio::test]
    async fn test_delete_permanent() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/old.txt");
        let node_id = register(&registry, &path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![node_id],
                trash: false,
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
                assert!(matches!(operation, OperationKind::Delete));
                assert!(success);
                assert_eq!(affected.len(), 1);
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        let deletes = provider.get_delete_calls();
        assert_eq!(deletes.len(), 1);
        assert_eq!(deletes[0], path);
    }

    #[tokio::test]
    async fn test_delete_to_trash() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/old.txt");
        let node_id = register(&registry, &path);

        let (trash_fn, trash_calls) = tracking_trash_fn();
        let (cmd_tx, evt_rx) = spawn_operator_with_trash(provider.clone(), registry, trash_fn);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![node_id],
                trash: true,
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
                assert!(matches!(operation, OperationKind::Delete));
                assert!(success);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        // Should have called trash_fn, not provider.delete()
        let trashed = trash_calls.lock().unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0], path);

        assert!(
            provider.get_delete_calls().is_empty(),
            "trash:true should use trash_fn, not provider.delete()"
        );
    }

    #[tokio::test]
    async fn test_delete_multiple_targets() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let p1 = PathBuf::from("/home/user/a.txt");
        let p2 = PathBuf::from("/home/user/b.txt");
        let p3 = PathBuf::from("/home/user/c.txt");

        let id1 = register(&registry, &p1);
        let id2 = register(&registry, &p2);
        let id3 = register(&registry, &p3);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![id1, id2, id3],
                trash: false,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationCompleteCompat {
                success, affected, ..
            } => {
                assert!(success);
                assert_eq!(affected.len(), 3);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        assert_eq!(provider.get_delete_calls().len(), 3);
    }

    #[tokio::test]
    async fn test_delete_error_emits_error_event() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/protected.txt");
        let node_id = register(&registry, &path);

        provider.add_fail_path(&path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![node_id],
                trash: false,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);
    }
}
