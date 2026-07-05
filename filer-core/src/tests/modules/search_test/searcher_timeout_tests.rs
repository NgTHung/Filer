#[cfg(test)]
mod searcher_timeout_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_times_out_with_provider_context() {
        let provider = MockProvider::new();
        provider.add_dir("/slow", vec![MockProvider::make_file("a.txt", "/slow", 1)]);
        // Far longer than the search timeout, so the deadline must fire first.
        provider.set_delay_ms(10_000);
        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) =
            spawn_searcher_with_timeout(provider, registry, Duration::from_millis(20));

        let session = SessionId::new();
        let request = RequestId::new();
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("a").unwrap(),
                root: LocationRef::from_location(&Location::local(PathBuf::from("/slow"))),
                event_mode: SearchEventMode::Compat,
                session,
                request,
            })
            .unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let error = loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::Error {
                    code,
                    target,
                    session: s,
                    ..
                })) if s == session => break (code, target),
                Ok(Ok(_)) => {}
                _ => panic!("timed out waiting for a search error event"),
            }
        };

        assert_eq!(error.0, ErrorCode::TimedOut);
        assert_eq!(error.1, Some(ErrorTarget::Provider("mock".to_string())));
    }
}
