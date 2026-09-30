#[cfg(test)]
mod copy_tests {
    use super::*;

    #[tokio::test]
    async fn test_copy_single_file() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/doc.txt");
        let dst_path = PathBuf::from("/home/user/backup");


        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/home/user", 1024),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation_id: id,
                operation,
                success,
                affected,
                session: s,
                ..
            } => {
                assert_eq!(id, operation_id);
                assert!(matches!(operation, OperationKind::Copy));
                assert!(success);
                assert!(!affected.is_empty());
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].0, src_path);
        assert_eq!(copies[0].1, dst_path.join("doc.txt"));
    }

    #[tokio::test]
    async fn test_copy_directory_recursive() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_dir = PathBuf::from("/home/user/project");
        let dst_dir = PathBuf::from("/home/user/backup");


        provider.add_metadata(&src_dir, MockOpsProvider::make_dir("project", "/home/user"));

        provider.add_dir_listing(
            &src_dir,
            vec![
                MockOpsProvider::make_file("a.txt", "/home/user/project", 100),
                MockOpsProvider::make_file("b.txt", "/home/user/project", 200),
                MockOpsProvider::make_file("c.txt", "/home/user/project", 300),
            ],
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_dir)],
                destination: local_ref(&dst_dir),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation: operation_id,
            })
            .unwrap();

        let (progress, final_event) = wait_for_completion(&evt_rx, session).await;

        // Should have ProgressUpdated events for each file copied.
        let progress_count = progress
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::ProgressUpdated {
                        scope,
                        ..
                    } if scope.session == session
                        && scope.operation == Some(operation_id)
                        && matches!(scope.kind, ProgressKind::Operation(OperationKind::Copy))
                )
            })
            .count();
        assert!(
            progress_count >= 3,
            "Recursive copy of 3 files should emit at least 3 progress events, got {progress_count}"
        );

        // Verify progress events have incrementing items_done
        let items_done_values: Vec<usize> = progress
            .iter()
            .filter_map(|e| match e {
                Event::ProgressUpdated { scope, snapshot }
                    if scope.session == session && scope.operation == Some(operation_id) =>
                {
                    Some(snapshot.done)
                }
                _ => None,
            })
            .collect();
        for window in items_done_values.windows(2) {
            assert!(
                window[1] >= window[0],
                "items_done should be non-decreasing: {items_done_values:?}"
            );
        }

        let mut targeted_progress = 0;
        for event in &progress {
            if let Event::ProgressUpdated { scope, snapshot }
                = event
                && scope.session == session
                && scope.operation == Some(operation_id)
                && matches!(scope.kind, ProgressKind::Operation(OperationKind::Copy))
                && snapshot.current.is_some()
            {
                targeted_progress += 1;
                assert!(
                    matches!(snapshot.current, Some(ProgressTarget::Location(_))),
                    "recursive copy progress should identify the destination by Location"
                );
            }
        }
        assert!(
            targeted_progress >= 3,
            "each recursive file should emit a Location progress target"
        );

        match final_event {
            Event::OperationComplete {
                operation,
                operation_id: id,
                success,
                ..
            } => {
                assert!(matches!(operation, OperationKind::Copy));
                assert_eq!(id, operation_id);
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        // All 3 files should have been copied
        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 3);

        // Destination directory should have been created
        let mkdirs = provider.get_mkdir_calls();
        assert!(!mkdirs.is_empty(), "Should create destination subdirectory");
    }

    #[tokio::test]
    async fn test_copy_multiple_sources() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src1 = PathBuf::from("/home/user/a.txt");
        let src2 = PathBuf::from("/home/user/b.txt");
        let dst = PathBuf::from("/home/user/backup");


        provider.add_metadata(
            &src1,
            MockOpsProvider::make_file("a.txt", "/home/user", 100),
        );
        provider.add_metadata(
            &src2,
            MockOpsProvider::make_file("b.txt", "/home/user", 200),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src1), local_ref(&src2)],
                destination: local_ref(&dst),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                success, affected, ..
            } => {
                assert!(success);
                assert_eq!(affected.len(), 2);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 2);
    }

    #[tokio::test]
    async fn test_copy_error_emits_error_event() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/locked.txt");
        let dst_path = PathBuf::from("/home/user/backup");


        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("locked.txt", "/home/user", 100),
        );
        provider.add_fail_path(&src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Location,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::Error {
                recoverable,
                session: s,
                request,
                operation,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(request, Some(request_id));
                assert_eq!(operation, Some(operation_id));
                assert!(recoverable, "PermissionDenied should be recoverable");
            }
            other => panic!("Expected Error event, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_copy_unresolvable_source_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let dst_path = PathBuf::from("/home/user/backup");

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![LocationRef::id_only(LocationId(404))],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Location,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);
    }
}
