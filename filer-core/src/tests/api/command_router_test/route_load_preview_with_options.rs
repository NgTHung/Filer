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
                path: p,
                options: o,
                session: s,
                ..
            } => {
                assert_eq!(
                    p, node,
                    "Preview request NodeId must match requested command"
                );
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
