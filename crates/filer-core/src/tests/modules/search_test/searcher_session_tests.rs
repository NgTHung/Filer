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
                let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("file").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: LocationRef::from_location(&Location::local("/root")),
                event_mode: SearchEventMode::Location,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::SearchResults {
                    session: s,
                    complete,
                    ..
                })) => {
                    assert_eq!(
                        s, session,
                        "SearchResults should carry the correct session ID"
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
