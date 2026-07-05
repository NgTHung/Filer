#[cfg(test)]
mod searcher_hidden_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_excludes_hidden_by_default() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("visible.txt", "/root", 100),
                MockProvider::make_hidden_file(".hidden", "/root", 200),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        // Default: include_hidden = false, no text filter so matches name
        let query = SearchQuery::parse("type:file").unwrap();
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
            "hidden files should be excluded by default"
        );
        assert_eq!(matches[0].name, "visible.txt");
    }

    #[tokio::test]
    async fn test_search_skips_hidden_directories() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("visible.txt", "/root", 100),
                MockProvider::make_hidden_dir(".git", "/root"),
            ],
        );
        provider.add_dir(
            "/root/.git",
            vec![
                MockProvider::make_file("HEAD", "/root/.git", 50),
                MockProvider::make_file("config", "/root/.git", 75),
            ],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider.clone(), registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("type:file").unwrap();
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
        assert_eq!(matches.len(), 1, "should not traverse into .git directory");
        assert_eq!(matches[0].name, "visible.txt");

        // Verify .git was never listed
        let calls = provider.list_calls();
        assert!(
            !calls.contains(&PathBuf::from("/root/.git")),
            "provider.list() should never be called on hidden directory"
        );
    }

    #[tokio::test]
    async fn test_search_includes_hidden_when_requested() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("visible.txt", "/root", 100),
                MockProvider::make_hidden_file(".env", "/root", 200),
                MockProvider::make_hidden_dir(".config", "/root"),
            ],
        );
        provider.add_dir(
            "/root/.config",
            vec![MockProvider::make_hidden_file(
                ".settings",
                "/root/.config",
                50,
            )],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        // hidden:yes enables include_hidden AND adds IsHidden filter (only hidden files match)
        let query = SearchQuery::parse("hidden:yes").unwrap();
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
        // Should find .env, .config dir, and .settings inside .config
        assert!(
            matches.len() >= 2,
            "should include hidden files and traverse hidden dirs"
        );
        assert!(
            matches.iter().all(|m| m.meta.hidden),
            "all results should be hidden"
        );
    }
}
