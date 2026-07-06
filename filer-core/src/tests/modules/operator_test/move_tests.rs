#[cfg(test)]
mod move_tests {
    use super::*;

    #[tokio::test]
    async fn test_move_same_filesystem_atomic() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/doc.txt");
        let dst_path = PathBuf::from("/home/user/archive");

        let _src_id = register(&registry, &src_path);
        let _dst_id = register(&registry, &dst_path);

        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/home/user", 1024),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (progress_events, final_event) = wait_for_completion(&evt_rx, session).await;

        // Same-FS move emits only completion progress before completion.
        let progress_count = progress_events
            .iter()
            .filter(|e| matches!(e, Event::ProgressUpdated { .. }))
            .count();
        assert_eq!(
            progress_count, 1,
            "Same-filesystem move should only emit completion progress"
        );
        assert!(progress_events.iter().any(|e| {
            matches!(
                e,
                Event::ProgressUpdated {
                    scope,
                    snapshot
                } if matches!(scope.kind, ProgressKind::Operation(OperationKind::Move))
                    && snapshot.status == ProgressStatus::Completed
            )
        }));

        match final_event {
            Event::OperationCompleteCompat {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Move));
                assert!(success);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        // Should have used rename, not copy
        let renames = provider.get_rename_calls();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].0, src_path);
        assert_eq!(renames[0].1, dst_path.join("doc.txt"));

        assert!(
            provider.get_copy_calls().is_empty(),
            "Same-FS move should not copy"
        );
    }

    #[tokio::test]
    async fn test_move_cross_filesystem_fallback() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/mnt/usb/doc.txt");
        let dst_path = PathBuf::from("/home/user/archive");

        let _src_id = register(&registry, &src_path);
        let _dst_id = register(&registry, &dst_path);

        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/mnt/usb", 1024),
        );
        provider.set_cross_device(true);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Compat,
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
                assert!(matches!(operation, OperationKind::Move));
                assert!(success);
            }
            other => panic!("Expected OperationCompleteCompat, got: {other:?}"),
        }

        // Should have fallen back to copy + delete
        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 1, "Cross-FS move should copy");

        let deletes = provider.get_delete_calls();
        assert_eq!(
            deletes.len(),
            1,
            "Cross-FS move should delete source after copy"
        );
        assert_eq!(deletes[0], src_path);
    }

    #[tokio::test]
    async fn test_move_error_emits_correlated_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/locked.txt");
        let dst_path = PathBuf::from("/home/user/archive");

        let _src_id = register(&registry, &src_path);
        let _dst_id = register(&registry, &dst_path);
        provider.add_fail_path(&src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Compat,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);
    }
}
