#[cfg(test)]
mod cancel_tests {
    use super::*;

    #[tokio::test]
    async fn test_copy_cancel_midway() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_dir = PathBuf::from("/home/user/big_project");
        let dst_dir = PathBuf::from("/home/user/backup");


        provider.add_metadata(
            &src_dir,
            MockOpsProvider::make_dir("big_project", "/home/user"),
        );

        let mut files = Vec::new();
        for i in 0..50 {
            files.push(MockOpsProvider::make_file(
                &format!("file_{i}.txt"),
                "/home/user/big_project",
                100,
            ));
        }
        provider.add_dir_listing(&src_dir, files);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_dir)],
                destination: local_ref(&dst_dir),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        // Give the operation a moment to start, then cancel
        tokio::task::yield_now().await;
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        let has_successful_complete = events.iter().any(|e| {
            matches!(
                e,
                Event::OperationComplete {
                    success: true,
                    session: s,
                    ..
                } if *s == session
            )
        });

        assert!(
            !has_successful_complete,
            "Cancelled copy should not emit OperationComplete with success"
        );

        // Should have copied fewer than 50 files
        let copies = provider.get_copy_calls();
        assert!(
            copies.len() < 50,
            "Cancel should stop before all files are copied (copied {})",
            copies.len()
        );
    }

    #[tokio::test]
    async fn test_move_cancel_midway() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/doc.txt");
        let dst_path = PathBuf::from("/home/user/backup");
        provider.set_rename_delay_ms(200);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let operation = OperationId::new();
        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref(&src_path)],
                destination: local_ref(&dst_path),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation,
            })
            .unwrap();

        tokio::task::yield_now().await;
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(300)).await;
        let has_successful_complete = events.iter().any(|event| {
            matches!(
                event,
                Event::OperationComplete {
                    operation_id,
                    success: true,
                    session: s,
                    ..
                } if *s == session && *operation_id == operation
            )
        });

        assert!(
            !has_successful_complete,
            "Cancelled move should not emit OperationComplete with success"
        );
    }

    #[tokio::test]
    async fn test_delete_cancel_midway() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/doc.txt");
        provider.set_delete_delay_ms(200);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let operation = OperationId::new();
        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![local_ref(&path)],
                trash: false,
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation,
            })
            .unwrap();

        tokio::task::yield_now().await;
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(300)).await;
        let has_successful_complete = events.iter().any(|event| {
            matches!(
                event,
                Event::OperationComplete {
                    operation_id,
                    success: true,
                    session: s,
                    ..
                } if *s == session && *operation_id == operation
            )
        });

        assert!(
            !has_successful_complete,
            "Cancelled delete should not emit OperationComplete with success"
        );
    }

    #[tokio::test]
    async fn test_cancel_operation_ignores_different_operation_id() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let operation = OperationId::new();

        let src_dir = PathBuf::from("/home/user/project");
        let dst_dir = PathBuf::from("/home/user/backup");


        provider.add_metadata(&src_dir, MockOpsProvider::make_dir("project", "/home/user"));

        let files = (0..10)
            .map(|i| {
                MockOpsProvider::make_file(&format!("file_{i}.txt"), "/home/user/project", 100)
            })
            .collect();
        provider.add_dir_listing(&src_dir, files);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_dir)],
                destination: local_ref(&dst_dir),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation,
            })
            .unwrap();

        tokio::task::yield_now().await;
        cmd_tx
            .send(OpsCommand::CancelOperation {
                session,
                operation: OperationId::new(),
            })
            .unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        let has_successful_complete = events.iter().any(|e| {
            matches!(
                e,
                Event::OperationComplete {
                    operation_id,
                    success: true,
                    session: s,
                    ..
                } if *s == session && *operation_id == operation
            )
        });

        assert!(
            has_successful_complete,
            "cancel for another operation id must not cancel the active operation"
        );
    }

    #[tokio::test]
    async fn test_session_destroy_cancels_operation() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_dir = PathBuf::from("/home/user/big_project");
        let dst_dir = PathBuf::from("/home/user/backup");


        provider.add_metadata(
            &src_dir,
            MockOpsProvider::make_dir("big_project", "/home/user"),
        );

        let mut files = Vec::new();
        for i in 0..50 {
            files.push(MockOpsProvider::make_file(
                &format!("file_{i}.txt"),
                "/home/user/big_project",
                100,
            ));
        }
        provider.add_dir_listing(&src_dir, files);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src_dir)],
                destination: local_ref(&dst_dir),
                event_mode: OperationEventMode::Location,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        tokio::task::yield_now().await;
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        let _events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        let copies = provider.get_copy_calls();
        assert!(
            copies.len() < 50,
            "Session destroy should cancel in-flight operation (copied {})",
            copies.len()
        );
    }
}
