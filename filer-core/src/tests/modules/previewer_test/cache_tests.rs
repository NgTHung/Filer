#[cfg(test)]
mod cache_tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_hit_skips_generation() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/cached.txt");
        let node_id = registry.clone().register(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, cache) = spawn_previewer(mock.clone(), registry);

        // Pre-populate cache
        cache.lock().unwrap().put(path, text_preview());

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReadyCompat { .. }));
        assert_eq!(
            mock.calls(),
            0,
            "Provider should not be called on cache hit"
        );
    }

    #[tokio::test]
    async fn test_cache_miss_calls_provider() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/uncached.txt");
        let node_id = registry.clone().register(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, _cache) = spawn_previewer(mock.clone(), registry);

        cmd_tx
            .send(PreviewCommand::Generate {
                path: node_id,
                options: None,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReadyCompat { .. }));
        assert_eq!(mock.calls(), 1);
    }

    #[tokio::test]
    async fn test_location_preview_cache_hit_skips_provider() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/location-cached.txt");
        let location = Location::local(path.clone());

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, cache) = spawn_previewer(mock.clone(), registry);

        cache.lock().unwrap().put(path, text_preview());

        cmd_tx
            .send(PreviewCommand::GenerateLocation {
                location: LocationRef::from_location(&location),
                options: None,
                session,
                request: RequestId::new(),
            })
            .unwrap();

        let event = wait_for_preview(&evt_rx, session).await;
        assert!(matches!(event, Event::PreviewReady { .. }));
        assert_eq!(
            mock.calls(),
            0,
            "Location preview should reuse direct-path cache entries"
        );
    }
}
