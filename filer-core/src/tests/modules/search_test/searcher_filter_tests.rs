#[cfg(test)]
mod searcher_filter_tests {
    use super::*;

    #[tokio::test]
    async fn test_filter_by_extension() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("main.rs", "/root", 100),
                MockProvider::make_file("lib.rs", "/root", 200),
                MockProvider::make_file("readme.md", "/root", 50),
                MockProvider::make_file("config.toml", "/root", 75),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("ext:rs").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2, "should match only .rs files");
        assert!(matches.iter().all(|m| m.name.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_filter_by_size_greater_than() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("small.txt", "/root", 100),
                MockProvider::make_file("medium.txt", "/root", 500),
                MockProvider::make_file("large.txt", "/root", 2000),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("size:>1000").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "large.txt");
    }

    #[tokio::test]
    async fn test_filter_by_size_less_than() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("small.txt", "/root", 100),
                MockProvider::make_file("medium.txt", "/root", 500),
                MockProvider::make_file("large.txt", "/root", 2000),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("size:<500").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "small.txt");
    }

    #[tokio::test]
    async fn test_filter_by_type_file() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("file.txt", "/root", 100),
                MockProvider::make_dir("subdir", "/root"),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("type:file").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert!(matches[0].is_file());
    }

    #[tokio::test]
    async fn test_filter_by_type_directory() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("file.txt", "/root", 100),
                MockProvider::make_dir("subdir", "/root"),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("type:dir").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert!(matches[0].is_dir());
    }

    #[tokio::test]
    async fn test_filter_is_hidden() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("visible.txt", "/root", 100),
                MockProvider::make_hidden_file(".secret", "/root", 200),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // hidden:yes enables include_hidden AND adds IsHidden filter
        let query = SearchQuery::parse("hidden:yes").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(
            matches.len(),
            1,
            "hidden:yes filter should only return hidden files"
        );
        assert_eq!(matches[0].name, ".secret");
    }

    #[tokio::test]
    async fn test_filter_name_contains() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("app_config.toml", "/root", 100),
                MockProvider::make_file("config.json", "/root", 200),
                MockProvider::make_file("readme.md", "/root", 50),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("name:config").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|m| m.name.contains("config")));
    }

    #[tokio::test]
    async fn test_filter_name_matches_regex() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("test_search.rs", "/root", 100),
                MockProvider::make_file("test_scan.rs", "/root", 200),
                MockProvider::make_file("main.rs", "/root", 300),
                MockProvider::make_file("test_nav.py", "/root", 400),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse(r"match:^test_.*\.rs$").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(
            matches.len(),
            2,
            "regex should match test_search.rs and test_scan.rs"
        );
        assert!(
            matches
                .iter()
                .all(|m| m.name.starts_with("test_") && m.name.ends_with(".rs"))
        );
    }

    #[tokio::test]
    async fn test_filter_modified_after() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                // modified at epoch + 100s (very old)
                MockProvider::make_file_with_time("old.txt", "/root", 50, 100),
                // modified at epoch + 2_000_000_000s (~2033)
                MockProvider::make_file_with_time("new.txt", "/root", 50, 2_000_000_000),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // after:1700000000 (~Nov 2023)
        let query = SearchQuery::parse("after:1700000000").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "new.txt");
    }

    #[tokio::test]
    async fn test_filter_modified_before() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file_with_time("old.txt", "/root", 50, 100),
                MockProvider::make_file_with_time("new.txt", "/root", 50, 2_000_000_000),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("before:1700000000").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "old.txt");
    }

    #[tokio::test]
    async fn test_multiple_filters_and_semantics() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("big.rs", "/root", 2000),
                MockProvider::make_file("small.rs", "/root", 50),
                MockProvider::make_file("big.py", "/root", 3000),
                MockProvider::make_file("tiny.rs", "/root", 10),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // Both filters must match (AND): .rs files AND size > 100
        let query = SearchQuery::parse("ext:rs size:>100").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(
            matches.len(),
            1,
            "only big.rs matches both ext:rs AND size:>100"
        );
        assert_eq!(matches[0].name, "big.rs");
    }
}
