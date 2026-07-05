#[cfg(test)]
mod searcher_basic_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_returns_matching_files() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("readme.md", "/root", 100),
                MockProvider::make_file("other.txt", "/root", 200),
                MockProvider::make_file("README.txt", "/root", 150),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("readme").unwrap();
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
        // Case insensitive by default: should match both "readme.md" and "README.txt"
        assert_eq!(
            matches.len(),
            2,
            "should match both readme files (case insensitive)"
        );
        assert!(
            matches
                .iter()
                .all(|m| m.name.to_lowercase().contains("readme"))
        );
    }

    #[tokio::test]
    async fn test_search_no_matches_returns_empty() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("hello.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("nonexistent").unwrap();
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
        assert!(
            matches.is_empty(),
            "should return empty when nothing matches"
        );
    }

    #[tokio::test]
    async fn test_search_path_returns_legacy_results() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("found.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("found").unwrap(),
                root: LocationRef::from_location(&Location::local(PathBuf::from("/root"))),
                event_mode: SearchEventMode::Compat,
                session,
                request: RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "found.txt");
    }

    #[tokio::test]
    async fn test_search_case_insensitive_by_default() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("readme.md", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("README").unwrap();
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
            1,
            "case insensitive: README should match readme.md"
        );
    }

    #[tokio::test]
    async fn test_search_case_sensitive() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("readme.md", "/root", 100),
                MockProvider::make_file("README.md", "/root", 200),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("README case:yes").unwrap();
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
            1,
            "case sensitive: only README.md should match"
        );
        assert_eq!(matches[0].name, "README.md");
    }
}
