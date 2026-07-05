#[cfg(test)]
mod searcher_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_unresolvable_root_emits_error() {
        let provider = MockProvider::new();
        let registry = NodeRegistry::new();
        // Don't register any path — root_id won't resolve
        let fake_root = crate::model::location::LocationId(99999);
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("anything").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: LocationRef::id_only(fake_root),
                event_mode: SearchEventMode::Compat,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // Should get an Error event
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::Error { session: s, .. })) if s == session => {
                    return; // Test passes — got expected error
                }
                Ok(Ok(Event::SearchResultsCompat {
                    session: s,
                    complete,
                    ..
                })) if s == session => {
                    if complete {
                        panic!("got SearchResultsCompat instead of Error for unresolvable root");
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) => panic!("channel closed"),
                Err(_) => panic!("timed out waiting for Error event"),
            }
        }
    }

    #[tokio::test]
    async fn test_search_skips_unreadable_directories() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("found.txt", "/root", 100),
                MockProvider::make_dir("readable", "/root"),
                MockProvider::make_dir("forbidden", "/root"),
            ],
        );
        provider.add_dir(
            "/root/readable",
            vec![MockProvider::make_file(
                "also_found.txt",
                "/root/readable",
                200,
            )],
        );
        // /root/forbidden will fail on list()
        provider.add_fail_path("/root/forbidden");

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("found").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: registry.resolve_node_location(root_id).unwrap(),
                event_mode: SearchEventMode::Compat,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(
            matches.len(),
            2,
            "should find files in readable dirs, skip forbidden"
        );

        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"found.txt"));
        assert!(names.contains(&"also_found.txt"));
    }
}
