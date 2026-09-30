#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_previewer_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded::<PreviewCommand>();
        let (evt_tx, _evt_rx) = flume::unbounded::<Event>();
        let cache = Arc::new(Mutex::new(PreviewCache::new(1024, Duration::from_secs(60))));
        let reg = Arc::new(PreviewRegistry::new());

        let previewer = Previewer::with_components(
            cmd_rx,
            evt_tx,
            Arc::new(NullProvider),
            NodeRegistry::new(),
            reg,
            cache,
        );
        let handle = tokio::spawn(async move { previewer.run().await });
        drop(cmd_tx);

        assert!(
            timeout(Duration::from_millis(500), handle).await.is_ok(),
            "Previewer should exit when command channel closes"
        );
    }
}
