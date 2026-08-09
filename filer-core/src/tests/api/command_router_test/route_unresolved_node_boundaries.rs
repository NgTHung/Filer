    async fn expect_request_error(
        harness: &RouterTestHarness,
        session: SessionId,
        request: RequestId,
    ) {
        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for boundary error")
            .expect("Event channel closed");

        match event {
            Event::Error {
                code,
                session: s,
                request: r,
                ..
            } => {
                assert_eq!(code, ErrorCode::InputInvalid);
                assert_eq!(s, session);
                assert_eq!(r, Some(request));
            }
            other => panic!("Expected Event::Error, got {other:?}"),
        }
    }

    async fn expect_operation_error(
        harness: &RouterTestHarness,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    ) {
        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for boundary error")
            .expect("Event channel closed");

        match event {
            Event::Error {
                code,
                session: s,
                request: r,
                operation: op,
                ..
            } => {
                assert_eq!(code, ErrorCode::InputInvalid);
                assert_eq!(s, session);
                assert_eq!(r, Some(request));
                assert_eq!(op, Some(operation));
            }
            other => panic!("Expected Event::Error, got {other:?}"),
        }
    }

    // Compatibility pin for API-006: unresolved NodeId scan requests must
    // remain rejected until the compatibility route becomes an absence test.
    #[tokio::test]
    async fn test_unresolved_scan_node_compat_does_not_reach_scanner() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::ScanNodeCompat {
                node: NodeId(4100),
                session,
                pipeline: PipelineConfig::default(),
                load: crate::DirectoryLoadOptions::default(),
                request,
            })
            .await;

        expect_request_error(&harness, session, request).await;
        assert!(harness.scan_rx.try_recv().is_err());
    }

    // Compatibility pin for API-006: unresolved NodeId search requests must
    // remain rejected until the compatibility route becomes an absence test.
    #[tokio::test]
    async fn test_unresolved_search_node_compat_does_not_reach_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::SearchNodeCompat {
                query: "needle".to_string(),
                root: NodeId(4101),
                session,
                request,
            })
            .await;

        expect_request_error(&harness, session, request).await;
        assert!(harness.search_rx.try_recv().is_err());
    }

    // Compatibility pin for API-006: unresolved NodeId metadata requests must
    // remain rejected until the compatibility route becomes an absence test.
    #[tokio::test]
    async fn test_unresolved_metadata_node_compat_does_not_reach_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::LoadMetadataNodeCompat {
                node: NodeId(4102),
                session,
                request,
            })
            .await;

        expect_request_error(&harness, session, request).await;
        assert!(harness.preview_rx.try_recv().is_err());
    }

    // Compatibility pin for API-006: unresolved NodeId extended-metadata
    // requests must remain rejected until the route becomes an absence test.
    #[tokio::test]
    async fn test_unresolved_extended_metadata_node_compat_does_not_reach_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::LoadExtendedMetadataNodeCompat {
                node: NodeId(4103),
                session,
                request,
            })
            .await;

        expect_request_error(&harness, session, request).await;
        assert!(harness.preview_rx.try_recv().is_err());
    }

    // Compatibility pin for API-006: unresolved NodeId watch requests must
    // remain rejected until the compatibility route becomes an absence test.
    #[tokio::test]
    async fn test_unresolved_watch_node_compat_does_not_reach_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness
            .send(Command::WatchNodeCompat {
                node: NodeId(4104),
                session,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for boundary error")
            .expect("Event channel closed");

        match event {
            Event::Error {
                code, session: s, ..
            } => {
                assert_eq!(code, ErrorCode::InputInvalid);
                assert_eq!(s, session);
            }
            other => panic!("Expected Event::Error, got {other:?}"),
        }
        assert!(harness.watch_rx.try_recv().is_err());
    }

    // Compatibility pin for API-006: unresolved NodeId operation requests must
    // remain rejected until the compatibility routes become absence tests.
    #[tokio::test]
    async fn test_unresolved_operation_node_compat_commands_do_not_reach_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let destination = harness.registry.clone().register(PathBuf::from("/ops/dst"));
        let missing = NodeId::from_path(&PathBuf::from("/ops/missing"));

        let copy = OperationId::new();
        let copy_request = RequestId::new();
        harness
            .send(Command::CopyNodeCompat {
                sources: vec![missing],
                destination,
                session,
                request: copy_request,
                operation: copy,
            })
            .await;
        expect_operation_error(&harness, session, copy_request, copy).await;

        let move_op = OperationId::new();
        let move_request = RequestId::new();
        harness
            .send(Command::MoveNodeCompat {
                sources: vec![missing],
                destination,
                session,
                request: move_request,
                operation: move_op,
            })
            .await;
        expect_operation_error(&harness, session, move_request, move_op).await;

        let delete = OperationId::new();
        let delete_request = RequestId::new();
        harness
            .send(Command::DeleteNodeCompat {
                nodes: vec![missing],
                trash: false,
                session,
                request: delete_request,
                operation: delete,
            })
            .await;
        expect_operation_error(&harness, session, delete_request, delete).await;

        let rename = OperationId::new();
        let rename_request = RequestId::new();
        harness
            .send(Command::RenameNodeCompat {
                node: missing,
                new_name: "renamed.txt".to_string(),
                session,
                request: rename_request,
                operation: rename,
            })
            .await;
        expect_operation_error(&harness, session, rename_request, rename).await;

        let create_folder = OperationId::new();
        let create_folder_request = RequestId::new();
        harness
            .send(Command::CreateFolderNodeCompat {
                parent: missing,
                name: "folder".to_string(),
                session,
                request: create_folder_request,
                operation: create_folder,
            })
            .await;
        expect_operation_error(&harness, session, create_folder_request, create_folder).await;

        let create_file = OperationId::new();
        let create_file_request = RequestId::new();
        harness
            .send(Command::CreateFileNodeCompat {
                parent: missing,
                name: "file.txt".to_string(),
                session,
                request: create_file_request,
                operation: create_file,
            })
            .await;
        expect_operation_error(&harness, session, create_file_request, create_file).await;

        assert!(harness.ops_rx.try_recv().is_err());
    }
