#[cfg(test)]
mod searcher_session_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_result_carries_correct_session() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("file.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("file").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: registry.resolve_node_location(root_id).unwrap(),
                event_mode: SearchEventMode::Compat,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::SearchResultsCompat {
                    session: s,
                    complete,
                    ..
                })) => {
                    assert_eq!(
                        s, session,
                        "SearchResultsCompat should carry the correct session ID"
                    );
                    if complete {
                        return;
                    }
                }
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed"),
            }
        }
    }
}
