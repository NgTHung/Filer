#[cfg(test)]
mod stale_event_tests {
    use super::*;

    #[tokio::test]
    async fn test_stale_preview_result_is_suppressed() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/stale-preview.txt");
        let node_id = registry.clone().register(path.clone());

        let mock = MockPreviewProvider::slow(text_preview(), 50);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(PreviewCommand::Generate {
                location: LocationRef::from_location(&Location::local(path.clone())),
                options: None,
                event_mode: PreviewEventMode::Compat { node: node_id },
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(PreviewCommand::Generate {
                location: LocationRef::from_location(&Location::local(path)),
                options: None,
                event_mode: PreviewEventMode::Compat { node: node_id },
                session,
                request: fresh_request,
            })
            .unwrap();

        let mut ready_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::PreviewReadyCompat {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                ready_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(ready_requests, vec![fresh_request]);
        assert!(
            !ready_requests.contains(&stale_request),
            "stale preview request should not emit PreviewReadyCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_location_preview_result_is_suppressed() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let location = Location::local("/tmp/stale-location-preview.txt");

        let mock = MockPreviewProvider::slow(text_preview(), 50);
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock, registry);
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(PreviewCommand::Generate {
                location: LocationRef::from_location(&location),
                options: None,
                event_mode: PreviewEventMode::Location,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(PreviewCommand::Generate {
                location: LocationRef::from_location(&location),
                options: None,
                event_mode: PreviewEventMode::Location,
                session,
                request: fresh_request,
            })
            .unwrap();

        let mut ready_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::PreviewReady {
                session: s,
                request,
                ..
            } = event
                && s == session
            {
                ready_requests.push(request);
                if request == fresh_request {
                    break;
                }
            }
        }

        assert_eq!(ready_requests, vec![fresh_request]);
        assert!(
            !ready_requests.contains(&stale_request),
            "stale Location preview request should not emit PreviewReady"
        );
    }
}
