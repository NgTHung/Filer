    // Compatibility pin for API-006: preserve NodeId copy routing until the
    // compatibility command is removed and replaced with an absence test.
    #[tokio::test]
    async fn test_route_copy_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let src = harness
            .registry
            .clone()
            .register(PathBuf::from("/a/file.txt"));
        let dst = harness.registry.clone().register(PathBuf::from("/b"));
        let src_location = harness.registry.resolve_node_location(src).unwrap();
        let dst_location = harness.registry.resolve_node_location(dst).unwrap();
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CopyNodeCompat {
                sources: vec![src],
                destination: dst,
                session,
                request,
                operation,
            })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::Copy {
                sources,
                destination,
                event_mode,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(sources, vec![src_location]);
                assert_eq!(destination, dst_location);
                assert_eq!(event_mode, OperationEventMode::Compat);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::Copy, got {:?}", other),
        }
    }

    // Compatibility pin for API-006: preserve NodeId create-file routing until
    // the compatibility command is removed and replaced with an absence test.
    #[tokio::test]
    async fn test_route_create_file_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let parent = harness
            .registry
            .clone()
            .register(PathBuf::from("/home/user"));
        let parent_location = harness.registry.resolve_node_location(parent).unwrap();
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CreateFileNodeCompat {
                parent,
                name: "notes.txt".to_string(),
                session,
                request,
                operation,
            })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::CreateFile {
                parent: p,
                name,
                event_mode,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(p, parent_location);
                assert_eq!(name, "notes.txt");
                assert_eq!(event_mode, OperationEventMode::Compat);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::CreateFile, got {:?}", other),
        }
    }
