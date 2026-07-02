#[cfg(test)]
mod searcher_traversal_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_traverses_subdirectories() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("a.txt", "/root", 100),
                MockProvider::make_dir("sub", "/root"),
            ],
        );
        provider.add_dir(
            "/root/sub",
            vec![MockProvider::make_file("b.txt", "/root/sub", 200)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("txt").unwrap();
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
            "should find files in both root and subdirectory"
        );

        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
    }

    #[tokio::test]
    async fn test_search_respects_max_depth() {
        let provider = MockProvider::new();
        // depth 0: /root
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("level0.txt", "/root", 10),
                MockProvider::make_dir("d1", "/root"),
            ],
        );
        // depth 1: /root/d1
        provider.add_dir(
            "/root/d1",
            vec![
                MockProvider::make_file("level1.txt", "/root/d1", 20),
                MockProvider::make_dir("d2", "/root/d1"),
            ],
        );
        // depth 2: /root/d1/d2
        provider.add_dir(
            "/root/d1/d2",
            vec![MockProvider::make_file("level2.txt", "/root/d1/d2", 30)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // depth:1 means traverse root (depth 0) and one level down (depth 1)
        let query = SearchQuery::parse("level depth:1").unwrap();
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
            "depth:1 should find level0.txt and level1.txt only"
        );

        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"level0.txt"));
        assert!(names.contains(&"level1.txt"));
        assert!(
            !names.contains(&"level2.txt"),
            "level2 should be excluded by depth limit"
        );
    }

    #[tokio::test]
    async fn test_search_bfs_order() {
        // BFS should return shallow matches before deep ones
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![
                MockProvider::make_file("shallow.rs", "/root", 100),
                MockProvider::make_dir("deep", "/root"),
            ],
        );
        provider.add_dir(
            "/root/deep",
            vec![MockProvider::make_dir("deeper", "/root/deep")],
        );
        provider.add_dir(
            "/root/deep/deeper",
            vec![MockProvider::make_file(
                "deep_file.rs",
                "/root/deep/deeper",
                200,
            )],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("rs").unwrap();
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
        // BFS: shallow.rs should appear before deep_file.rs
        assert_eq!(
            matches[0].name, "shallow.rs",
            "BFS: shallow match should come first"
        );
        assert_eq!(
            matches[1].name, "deep_file.rs",
            "BFS: deep match should come second"
        );
    }
}
