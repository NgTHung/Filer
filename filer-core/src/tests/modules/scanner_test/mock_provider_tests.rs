#[cfg(test)]
mod mock_provider_tests {
    use super::*;

    #[test]
    fn test_mock_provider_capabilities() {
        let provider = MockProvider::new();
        let caps = provider.capabilities();

        assert!(caps.read);
        assert!(!caps.write);
        assert!(!caps.watch);
        assert!(!caps.search);
    }

    #[test]
    fn test_mock_provider_scheme() {
        let provider = MockProvider::new();
        assert_eq!(provider.scheme(), "mock");
    }

    #[tokio::test]
    async fn test_mock_provider_list_success() {
        let provider = MockProvider::new();

        // provider.add_file(FileNode {
        //     id: 1.into(),
        //     name: "test.txt".to_string(),
        //     path: PathBuf::from("/test.txt"),
        //     is_dir: false,
        //     size: 100,
        //     modified: None,
        //     created: None,
        //     permissions: None,
        //     metadata: None,
        // });
        provider.add_file(make_file("test.txt", "/test", 100, false));

        let result = provider
            .list(Path::new("/test"), &crate::ProviderCx::none())
            .await;

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "test.txt");
    }

    #[tokio::test]
    async fn test_mock_provider_tracks_calls() {
        let provider = MockProvider::new();

        provider
            .list(Path::new("/dir1"), &crate::ProviderCx::none())
            .await
            .unwrap();
        provider
            .list(Path::new("/dir2"), &crate::ProviderCx::none())
            .await
            .unwrap();

        let calls = provider.get_list_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], PathBuf::from("/dir1"));
        assert_eq!(calls[1], PathBuf::from("/dir2"));
    }

    #[tokio::test]
    async fn test_mock_provider_can_fail() {
        let provider = MockProvider::new();
        provider.set_should_fail(true);

        let result = provider
            .list(Path::new("/test"), &crate::ProviderCx::none())
            .await;
        assert!(result.is_err());
    }
}
