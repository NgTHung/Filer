#[cfg(test)]
mod cancel_tests {
    use super::*;

    struct SequencedPreviewProvider {
        result: PreviewData,
        delays: Mutex<Vec<Duration>>,
    }

    impl SequencedPreviewProvider {
        fn new(result: PreviewData, delays: Vec<Duration>) -> Self {
            Self {
                result,
                delays: Mutex::new(delays),
            }
        }
    }

    #[async_trait]
    impl PreviewProvider for SequencedPreviewProvider {
        fn supported_categories(&self) -> &[MimeCategory] {
            &[MimeCategory::Text]
        }

        async fn generate(
            &self,
            _path: &Path,
            _mime: &MimeInfo,
            _options: &PreviewOptions,
        ) -> Result<PreviewData, CoreError> {
            let delay = self
                .delays
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(Duration::ZERO);
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            Ok(self.result.clone())
        }

        fn name(&self) -> &'static str {
            "sequenced"
        }
    }

    #[tokio::test]
    async fn test_cancel_prevents_event_emission() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/slow.txt");
        let node_id = registry.clone().register(path.clone());

        // Provider takes 200ms — plenty of time to cancel
        let mock = MockPreviewProvider::slow(text_preview(), 200);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        // Collect all events for 300ms — should see nothing for our session
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(e)) => events.push(e),
                _ => break,
            }
        }

        let session_events: Vec<_> = events
            .iter()
            .filter(|e| match e {
                Event::PreviewReadyCompat { session: s, .. }
                | Event::PreviewFailedCompat { session: s, .. } => *s == session,
                _ => false,
            })
            .collect();

        assert!(
            session_events.is_empty(),
            "Cancelled preview should not emit PreviewReadyCompat or PreviewFailedCompat"
        );
    }

    #[tokio::test]
    async fn test_cancel_location_preview_prevents_event_emission() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/slow-location.txt");

        let mock = MockPreviewProvider::slow(text_preview(), 200);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);

        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: RequestId::new(),
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(e)) => events.push(e),
                _ => break,
            }
        }

        let session_events: Vec<_> = events
            .iter()
            .filter(|e| match e {
                Event::PreviewReadyCompat { session: s, .. }
                | Event::PreviewFailedCompat { session: s, .. } => *s == session,
                _ => false,
            })
            .collect();

        assert!(
            session_events.is_empty(),
            "Cancelled Location preview should not emit PreviewReadyCompat or PreviewFailedCompat"
        );
    }

    #[tokio::test]
    async fn test_generate_rapid_reissue_then_cancel_cancels_fresh_preview() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/rapid-preview.txt");
        let node_id = registry.clone().register(path);
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        let mut preview_reg = PreviewRegistry::new();
        preview_reg.register(Box::new(SequencedPreviewProvider::new(
            text_preview(),
            vec![Duration::from_millis(120), Duration::from_millis(10)],
        )));
        let (cmd_tx, evt_rx, _cache) =
            spawn_previewer_with_provider(Arc::new(NullProvider), Arc::new(preview_reg), registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: fresh_request,
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(180);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::PreviewReadyCompat {
                    session: s,
                    request,
                    ..
                }
                | Event::PreviewFailedCompat {
                    session: s,
                    request,
                    ..
                } if s == session && request == fresh_request => {
                    panic!("fresh preview emitted after rapid reissue cancellation: {event:?}");
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_generate_location_rapid_reissue_then_cancel_cancels_fresh_preview() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/rapid-location-preview.txt");
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        let mut preview_reg = PreviewRegistry::new();
        preview_reg.register(Box::new(SequencedPreviewProvider::new(
            text_preview(),
            vec![Duration::from_millis(120), Duration::from_millis(10)],
        )));
        let (cmd_tx, evt_rx, _cache) =
            spawn_previewer_with_provider(Arc::new(NullProvider), Arc::new(preview_reg), registry);

        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: fresh_request,
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(180);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::PreviewReady {
                    session: s,
                    request,
                    ..
                }
                | Event::PreviewFailed {
                    session: s,
                    request,
                    ..
                } if s == session && request == fresh_request => {
                    panic!(
                        "fresh location preview emitted after rapid reissue cancellation: {event:?}"
                    );
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_generate_passes_cancel_context_to_mime_fallback() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/context-preview.txt");
        let node_id = registry.clone().register(path);

        let saw_cancel = Arc::new(Mutex::new(false));
        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: saw_cancel.clone(),
            metadata_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: false,
            block_metadata: false,
        });
        let mut preview_reg = PreviewRegistry::new();
        preview_reg.register(Box::new(MockPreviewProvider::instant(text_preview())));
        let (cmd_tx, evt_rx, _cache) =
            spawn_previewer_with_provider(provider, Arc::new(preview_reg), registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReadyCompat { .. }));
        assert!(
            *saw_cancel.lock().unwrap(),
            "preview MIME fallback must receive a cancel-aware ProviderCx"
        );
    }

    #[tokio::test]
    async fn test_cancel_extended_metadata_interrupts_blocked_provider_read() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/context-metadata.txt");
        let node_id = registry.clone().register(path);

        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: true,
            block_metadata: false,
        });
        let (cmd_tx, evt_rx, _cache) = spawn_previewer_with_provider(
            provider,
            Arc::new(PreviewRegistry::new()),
            registry,
        );

        cmd_tx
            .send(PreviewCommand::LoadExtendedMetadata(
                node_id,
                session,
                RequestId::new(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::ExtendedMetadataLoadedCompat { session: s, .. }
                | Event::ExtendedMetadataLoaded { session: s, .. }
                | Event::Error { session: s, .. }
                    if s == session =>
                {
                    panic!("cancelled extended metadata emitted event: {event:?}");
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_cancel_extended_metadata_location_interrupts_blocked_provider_read() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/context-location-metadata.txt");

        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: true,
            block_metadata: false,
        });
        let (cmd_tx, evt_rx, _cache) = spawn_previewer_with_provider(
            provider,
            Arc::new(PreviewRegistry::new()),
            registry,
        );

        cmd_tx
            .send(PreviewCommand::LoadExtendedMetadataLocation(
                LocationRef::from_location(&location),
                session,
                RequestId::new(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::ExtendedMetadataLoadedCompat { session: s, .. }
                | Event::ExtendedMetadataLoaded { session: s, .. }
                | Event::Error { session: s, .. }
                    if s == session =>
                {
                    panic!("cancelled location extended metadata emitted event: {event:?}");
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_cancel_metadata_interrupts_blocked_provider_metadata() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/basic-metadata.txt");
        let node_id = registry.clone().register(path);
        let metadata_saw_cancel = Arc::new(Mutex::new(false));

        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_saw_cancel: metadata_saw_cancel.clone(),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: false,
            block_metadata: true,
        });
        let (cmd_tx, evt_rx, _cache) = spawn_previewer_with_provider(
            provider,
            Arc::new(PreviewRegistry::new()),
            registry,
        );

        cmd_tx
            .send(PreviewCommand::LoadMetadata(
                node_id,
                session,
                RequestId::new(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::MetadataLoadedCompat { session: s, .. } | Event::Error { session: s, .. }
                    if s == session =>
                {
                    panic!("cancelled metadata emitted event: {event:?}");
                }
                _ => {}
            }
        }

        assert!(
            *metadata_saw_cancel.lock().unwrap(),
            "metadata provider call must receive a cancel-aware ProviderCx"
        );
    }

    #[tokio::test]
    async fn test_cancel_metadata_location_interrupts_blocked_provider_metadata() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/basic-location-metadata.txt");
        let metadata_saw_cancel = Arc::new(Mutex::new(false));

        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_saw_cancel: metadata_saw_cancel.clone(),
            metadata_calls: Arc::new(Mutex::new(0)),
            block_reads: false,
            block_metadata: true,
        });
        let (cmd_tx, evt_rx, _cache) = spawn_previewer_with_provider(
            provider,
            Arc::new(PreviewRegistry::new()),
            registry,
        );

        cmd_tx
            .send(PreviewCommand::LoadMetadataLocation(
                LocationRef::from_location(&location),
                session,
                RequestId::new(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx.send(PreviewCommand::Cancel(session)).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            match event {
                Event::MetadataLoaded { session: s, .. } | Event::Error { session: s, .. }
                    if s == session =>
                {
                    panic!("cancelled metadata location emitted event: {event:?}");
                }
                _ => {}
            }
        }

        assert!(
            *metadata_saw_cancel.lock().unwrap(),
            "metadata location provider call must receive a cancel-aware ProviderCx"
        );
    }
}
