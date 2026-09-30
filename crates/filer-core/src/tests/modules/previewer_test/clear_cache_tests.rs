#[cfg(test)]
mod clear_cache_tests {
    use super::*;

    #[tokio::test]
    async fn test_clear_cache_causes_cache_miss() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let path = PathBuf::from("/tmp/cached2.txt");

        let mock = MockPreviewProvider::instant(text_preview());
        let (cmd_tx, evt_rx, cache) = spawn_previewer(mock.clone(), registry);

        // Pre-populate cache
        cache.lock().unwrap().put(path.clone(), text_preview());

        // Clear the cache
        cmd_tx.send(PreviewCommand::ClearCache).unwrap();
        tokio::task::yield_now().await;

        // Now generate — should miss cache and call provider
        let session2 = SessionId::new();
        cmd_tx
            .send(PreviewCommand::Generate {
                location: LocationRef::from_location(&Location::local(path)),
                options: None,
                event_mode: PreviewEventMode::Location,
                session: session2,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let _ = wait_for_preview(&evt_rx, session2).await;
        assert_eq!(
            mock.calls(),
            1,
            "Provider should be called after cache clear"
        );

        let _ = session; // suppress unused warning
    }
}
