#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_operator_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded::<OpsCommand>();
        let (evt_tx, _evt_rx) = flume::unbounded::<Event>();
        let provider = Arc::new(MockOpsProvider::new());
        let registry = NodeRegistry::new();

        let operator = Operator::with_trash_fn(cmd_rx, evt_tx, provider, registry, noop_trash_fn());
        let handle = tokio::spawn(async move {
            operator.run().await;
        });

        drop(cmd_tx);

        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(
            result.is_ok(),
            "Operator should exit when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_session_does_not_crash() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let (cmd_tx, _evt_rx) = spawn_operator(provider, registry);

        let session = SessionId::new();
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !cmd_tx.is_disconnected(),
            "Operator should still be alive after cancelling unknown session"
        );
    }
}
