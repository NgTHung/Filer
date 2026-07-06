#[cfg(test)]
mod cache_invalidation_tests {
    use super::*;

    fn new_cache() -> SharedDirCache {
        Arc::new(Mutex::new(DirCache::new(1024 * 1024)))
    }

    fn seed_cache(cache: &SharedDirCache, path: impl Into<PathBuf>) {
        cache.lock().unwrap().put(
            path.into(),
            ListingOptions::fast(),
            vec![MockOpsProvider::make_file("cached.txt", "/cache", 1)],
        );
    }

    fn assert_cached(cache: &SharedDirCache, path: impl Into<PathBuf>) {
        let path = path.into();
        assert!(
            cache
                .lock()
                .unwrap()
                .get(&path, ListingOptions::fast())
                .is_some(),
            "expected {} to remain cached",
            path.display()
        );
    }

    fn assert_invalidated(cache: &SharedDirCache, path: impl Into<PathBuf>) {
        let path = path.into();
        assert!(
            cache
                .lock()
                .unwrap()
                .get(&path, ListingOptions::fast())
                .is_none(),
            "expected {} to be invalidated",
            path.display()
        );
    }

    #[tokio::test]
    async fn test_create_file_invalidates_parent_cache() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let parent = PathBuf::from("/home/user");
        let _parent_id = register(&registry, &parent);
        seed_cache(&cache, &parent);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::CreateFile {
                parent: local_ref(&parent),
                name: "new.txt".to_string(),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_invalidated(&cache, parent);
    }

    #[tokio::test]
    async fn test_create_folder_invalidates_parent_cache() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let parent = PathBuf::from("/home/user");
        let _parent_id = register(&registry, &parent);
        seed_cache(&cache, &parent);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::CreateFolder {
                parent: local_ref(&parent),
                name: "new-folder".to_string(),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_invalidated(&cache, parent);
    }

    #[tokio::test]
    async fn test_copy_file_invalidates_destination_parent_only() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let src = PathBuf::from("/home/user/doc.txt");
        let src_parent = PathBuf::from("/home/user");
        let dst_parent = PathBuf::from("/home/user/backup");
        let _src_id = register(&registry, &src);
        let _dst_id = register(&registry, &dst_parent);
        provider.add_metadata(&src, MockOpsProvider::make_file("doc.txt", "/home/user", 1));
        seed_cache(&cache, &src_parent);
        seed_cache(&cache, &dst_parent);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![local_ref(&src)],
                destination: local_ref(&dst_parent),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_cached(&cache, src_parent);
        assert_invalidated(&cache, dst_parent);
    }

    #[tokio::test]
    async fn test_move_file_invalidates_source_and_destination_parents() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let src = PathBuf::from("/home/user/doc.txt");
        let src_parent = PathBuf::from("/home/user");
        let dst_parent = PathBuf::from("/mnt/archive");
        let _src_id = register(&registry, &src);
        let _dst_id = register(&registry, &dst_parent);
        seed_cache(&cache, &src_parent);
        seed_cache(&cache, &dst_parent);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref(&src)],
                destination: local_ref(&dst_parent),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_invalidated(&cache, src_parent);
        assert_invalidated(&cache, dst_parent);
    }

    #[tokio::test]
    async fn test_delete_directory_invalidates_parent_and_cached_subtree() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let parent = PathBuf::from("/home/user");
        let dir = PathBuf::from("/home/user/project");
        let child = PathBuf::from("/home/user/project/src");
        let sibling = PathBuf::from("/home/user/project-old");
        let _dir_id = register(&registry, &dir);
        seed_cache(&cache, &parent);
        seed_cache(&cache, &dir);
        seed_cache(&cache, &child);
        seed_cache(&cache, &sibling);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![local_ref(&dir)],
                trash: false,
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_invalidated(&cache, parent);
        assert_invalidated(&cache, dir);
        assert_invalidated(&cache, child);
        assert_cached(&cache, sibling);
    }

    #[tokio::test]
    async fn test_move_directory_invalidates_old_subtree_and_both_parents() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let src_parent = PathBuf::from("/home/user");
        let dst_parent = PathBuf::from("/mnt/archive");
        let dir = PathBuf::from("/home/user/project");
        let child = PathBuf::from("/home/user/project/src");
        let _dir_id = register(&registry, &dir);
        let _dst_id = register(&registry, &dst_parent);
        seed_cache(&cache, &src_parent);
        seed_cache(&cache, &dst_parent);
        seed_cache(&cache, &dir);
        seed_cache(&cache, &child);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![local_ref(&dir)],
                destination: local_ref(&dst_parent),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_invalidated(&cache, src_parent);
        assert_invalidated(&cache, dst_parent);
        assert_invalidated(&cache, dir);
        assert_invalidated(&cache, child);
    }

    #[tokio::test]
    async fn test_rename_directory_invalidates_parent_and_old_subtree() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let parent = PathBuf::from("/home/user");
        let dir = PathBuf::from("/home/user/project");
        let child = PathBuf::from("/home/user/project/src");
        let _dir_id = register(&registry, &dir);
        seed_cache(&cache, &parent);
        seed_cache(&cache, &dir);
        seed_cache(&cache, &child);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::Rename {
                source: local_ref(&dir),
                new_name: "renamed".to_string(),
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(
            final_event,
            Event::OperationCompleteCompat { success: true, .. }
        ));
        assert_invalidated(&cache, parent);
        assert_invalidated(&cache, dir);
        assert_invalidated(&cache, child);
    }

    #[tokio::test]
    async fn test_failed_delete_leaves_cache_intact() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let cache = new_cache();
        let session = SessionId::new();
        let parent = PathBuf::from("/home/user");
        let path = PathBuf::from("/home/user/protected.txt");
        let _id = register(&registry, &path);
        provider.add_fail_path(&path);
        seed_cache(&cache, &parent);

        let (cmd_tx, evt_rx) = spawn_operator_with_cache(provider, registry, cache.clone());
        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![local_ref(&path)],
                trash: false,
                event_mode: OperationEventMode::Compat,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert!(matches!(final_event, Event::Error { .. }));
        assert_cached(&cache, parent);
    }
}
