#[cfg(test)]
mod searcher_limit_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_respects_max_results() {
        let provider = MockProvider::new();
        let mut files = Vec::new();
        for i in 0..10 {
            files.push(MockProvider::make_file(
                &format!("file{}.txt", i),
                "/root",
                (i + 1) * 100,
            ));
        }
        provider.add_dir("/root", files);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("file max:3").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 3, "max:3 should return exactly 3 results");
    }

    #[tokio::test]
    async fn test_search_streaming_batches() {
        // Create enough files to trigger multiple batches (>50)
        let provider = MockProvider::new();
        let mut files = Vec::new();
        for i in 0..75 {
            files.push(MockProvider::make_file(
                &format!("item{:03}.txt", i),
                "/root",
                (i + 1) * 10,
            ));
        }
        provider.add_dir("/root", files);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("item").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // Count the number of SearchResultsCompat events
        let mut batch_count = 0;
        let mut total_matches = 0;
        let deadline = tokio::time::Instant::now() + TIMEOUT;

        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::SearchResultsCompat {
                    matches,
                    complete,
                    session: s,
                    ..
                })) if s == session => {
                    batch_count += 1;
                    total_matches += matches.len();
                    if complete {
                        break;
                    }
                }
                Ok(Ok(_)) => {}
                _ => panic!("timed out waiting for search batches"),
            }
        }

        assert_eq!(total_matches, 75, "should find all 75 items across batches");
        assert!(
            batch_count >= 2,
            "75 results should produce at least 2 batches (batch size ~50)"
        );
    }
}
