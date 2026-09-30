#[cfg(test)]
mod searcher_lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_searcher_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
        let (evt_tx, _evt_rx) = flume::unbounded::<Event>();
        let provider = Arc::new(MockProvider::new());
        let registry = NodeRegistry::new();

        let searcher = Searcher::new(cmd_rx, evt_tx, provider, registry);
        let handle = tokio::spawn(async move {
            searcher.run().await;
        });

        // Drop command sender — actor should exit gracefully
        drop(cmd_tx);

        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(
            result.is_ok(),
            "Searcher should exit when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_searcher_cancel_nonexistent_session() {
        let (cmd_tx, evt_rx) = {
            let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
            let (evt_tx, evt_rx) = flume::unbounded::<Event>();
            let provider = Arc::new(MockProvider::new());
            let registry = NodeRegistry::new();
            let searcher = Searcher::new(cmd_rx, evt_tx, provider, registry);
            tokio::spawn(async move {
                searcher.run().await;
            });
            (cmd_tx, evt_rx)
        };

        // Cancel a session that doesn't exist — should not crash
        cmd_tx
            .send(SearchCommand::Cancel(SessionId::new()))
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // No events should be emitted
        assert!(
            evt_rx.try_recv().is_err(),
            "No events expected for cancel of nonexistent session"
        );

        drop(cmd_tx);
    }
}
