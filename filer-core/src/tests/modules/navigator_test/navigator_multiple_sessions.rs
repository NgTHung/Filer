    #[tokio::test]
    async fn test_navigator_multiple_sessions() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session1 = session(1);
        let session2 = session(2);

        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session: session1,
                location: LocationRef::from_location(&Location::local("/tmp/session-one")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx
            .send(NavCommand::NavigateToLocation {
                session: session2,
                location: LocationRef::from_location(&Location::local("/tmp/session-two")),
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Both sessions should be independent
        cmd_tx
            .send(NavCommand::Back(
                session1,
                crate::model::request::RequestId::new(),
            ))
            .unwrap();
        cmd_tx.send(NavCommand::GetState(session2)).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should process without panic
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_handles_set_selected_command() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, _scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg);

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);
        // Compatibility pin for API-006: selection is still represented by NodeId.
        let nodes = vec![node(1), node(2), node(3)];

        // Set selection
        cmd_tx
            .send(NavCommand::SetSelected { session, nodes })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should process without panic
        assert!(!cmd_tx.is_disconnected());
    }

    #[tokio::test]
    async fn test_navigator_command_processing_order() {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (scanner_tx, scanner_rx) = flume::unbounded();
        let reg = NodeRegistry::new();
        let navigator = Navigator::new(cmd_rx, event_tx, scanner_tx, reg.clone());

        tokio::spawn(async move {
            navigator.run().await;
        });

        let session = session(1);

        let locations = [
            Location::local("/tmp/order-one"),
            Location::local("/tmp/order-two"),
            Location::local("/tmp/order-three"),
        ];
        for location in &locations {
            cmd_tx
                .send(NavCommand::NavigateToLocation {
                    session,
                    location: LocationRef::from_location(location),
                    request: crate::model::request::RequestId::new(),
                })
                .unwrap();
        }

        // Should receive scan commands in order
        for expected_location in &locations {
            let scan_cmd = timeout(Duration::from_millis(100), scanner_rx.recv_async())
                .await
                .expect("Should receive scan command")
                .expect("Channel should not be closed");

            match scan_cmd {
                ScanCommand::ScanLocation { location, .. } => {
                    assert_eq!(location.descriptor(), Some(expected_location.descriptor()));
                }
                _ => panic!("Expected ScanLocation command"),
            }
        }
    }
