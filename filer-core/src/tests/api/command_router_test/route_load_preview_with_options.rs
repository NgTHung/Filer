    #[tokio::test]
    async fn test_route_load_preview_with_options() {
        use crate::PreviewOptions;

        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/image.png");
        let node = harness.registry.clone().register(path.clone());

        let options = PreviewOptions {
            max_width: 800,
            max_height: 600,
            ..PreviewOptions::default()
        };

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: node,
                options: Some(options.clone()),
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match preview_cmd {
            PreviewCommand::Generate {
                location,
                options: o,
                event_mode,
                session: s,
                ..
            } => {
                assert_eq!(
                    location,
                    LocationRef::from_location(&Location::local(path)),
                    "Preview request location must resolve from requested NodeId"
                );
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(
                    s, session,
                    "Preview request session id must match requested command"
                );
                assert!(o.is_some(), "Options should be forwarded");
                let opts = o.unwrap();
                assert_eq!(opts.max_width, 800);
                assert_eq!(opts.max_height, 600);
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_metadata_node_compat_resolves_location() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/info.txt");
        let node = harness.registry.clone().register(path.clone());
        let request = RequestId::new();

        harness
            .send(Command::LoadMetadataNodeCompat {
                node,
                session,
                request,
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match preview_cmd {
            PreviewCommand::LoadMetadata {
                location,
                event_mode,
                session: s,
                request: r,
            } => {
                assert_eq!(location, LocationRef::from_location(&Location::local(path)));
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session);
                assert_eq!(r, request);
            }
            other => panic!("Expected PreviewCommand::LoadMetadata, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_extended_metadata_node_compat_resolves_location() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/info.png");
        let node = harness.registry.clone().register(path.clone());
        let request = RequestId::new();

        harness
            .send(Command::LoadExtendedMetadataNodeCompat {
                node,
                session,
                request,
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match preview_cmd {
            PreviewCommand::LoadExtendedMetadata {
                location,
                event_mode,
                session: s,
                request: r,
            } => {
                assert_eq!(location, LocationRef::from_location(&Location::local(path)));
                assert_eq!(event_mode, PreviewEventMode::Compat { node });
                assert_eq!(s, session);
                assert_eq!(r, request);
            }
            other => panic!(
                "Expected PreviewCommand::LoadExtendedMetadata, got {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_route_unresolved_preview_node_compat_emits_error() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: NodeId(404),
                options: None,
                session,
                request,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        assert!(matches!(
            event,
            Event::Error {
                session: s,
                request: Some(r),
                ..
            } if s == session && r == request
        ));
        assert!(harness.preview_rx.try_recv().is_err());
    }
