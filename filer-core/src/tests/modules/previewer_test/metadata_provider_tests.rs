#[cfg(test)]
mod metadata_provider_tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_metadata_uses_provider_metadata() {
        let registry = NodeRegistry::new();
        let session = SessionId::new();
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let node_id = registry.clone().register(path);

        let metadata_calls = Arc::new(Mutex::new(0));
        let provider = Arc::new(RecordingProvider {
            read_header_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_saw_cancel: Arc::new(Mutex::new(false)),
            metadata_calls: metadata_calls.clone(),
            block_reads: false,
            block_metadata: false,
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

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::MetadataLoadedCompat { session: s, .. })) if s == session => break,
                Ok(Ok(Event::Error { session: s, .. })) if s == session => {
                    panic!("metadata load failed")
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) => panic!("event channel closed"),
                Err(_) => panic!("timed out waiting for metadata event"),
            }
        }

        assert_eq!(*metadata_calls.lock().unwrap(), 1);
    }
}
