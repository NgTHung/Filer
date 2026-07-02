    #[tokio::test]
    async fn test_stale_scan_location_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.txt", "/tmp/location-stale", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/tmp/location-stale");

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryLoaded {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale ScanLocation request should not emit DirectoryLoaded"
        );
    }

    #[tokio::test]
    async fn test_stale_scan_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.txt", "/tmp/stale", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let path = PathBuf::from("/tmp/stale");

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline: default_pipeline(),
                load: snapshot_load(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryLoadedCompat {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale scan request should not emit DirectoryLoadedCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_scan_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.txt", "/tmp/stale-page", 10, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let path = PathBuf::from("/tmp/stale-page");

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoadedCompat {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale scan request should not emit DirectoryPageLoadedCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_filtered_scan_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file("fresh.rs", "/tmp/filter-stale-page", 10, false));
        provider.add_file(make_file("skip.txt", "/tmp/filter-stale-page", 20, false));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let path = PathBuf::from("/tmp/filter-stale-page");
        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));

        cmd_tx
            .send(ScanCommand::Scan {
                path: path.clone(),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::Scan {
                path,
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoadedCompat {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale filtered scan request should not emit DirectoryPageLoadedCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_scan_location_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file(
            "fresh.txt",
            "/tmp/location-stale-page",
            10,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/tmp/location-stale-page");

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: default_pipeline(),
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoaded {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale ScanLocation request should not emit DirectoryPageLoaded"
        );
    }

    #[tokio::test]
    async fn test_stale_filtered_scan_location_page_result_is_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_file(make_file(
            "fresh.rs",
            "/tmp/location-filter-stale-page",
            10,
            false,
        ));
        provider.add_file(make_file(
            "skip.txt",
            "/tmp/location-filter-stale-page",
            20,
            false,
        ));

        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();

        let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
        tokio::spawn(async move { scanner.run().await });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/tmp/location-filter-stale-page");
        let pipeline =
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));

        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::default(),
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(ScanCommand::ScanLocation {
                location: LocationRef::from_location(&location),
                session,
                pipeline,
                load: crate::DirectoryLoadOptions::default(),
                request: fresh_request,
            })
            .unwrap();

        let mut loaded_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::DirectoryPageLoaded {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                loaded_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(loaded_requests, vec![fresh_request]);
        assert!(
            !loaded_requests.contains(&stale_request),
            "stale filtered ScanLocation request should not emit DirectoryPageLoaded"
        );
    }
