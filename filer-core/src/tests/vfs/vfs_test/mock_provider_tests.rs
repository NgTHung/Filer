pub struct MockFs {
    files: HashMap<PathBuf, Vec<u8>>,
    directories: Vec<PathBuf>,
}

impl MockFs {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            directories: Vec::new(),
        }
    }

    pub fn add_file(&mut self, path: PathBuf, content: Vec<u8>) {
        self.files.insert(path, content);
    }

    // pub fn add_directory(&mut self, path: PathBuf) {
    //     self.directories.push(path);
    // }
}

#[async_trait]
impl FsProvider for MockFs {
    fn scheme(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: true,
            watch: false,
            search: true,
        }
    }

    async fn list(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        if !self.directories.contains(&path.to_path_buf()) {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let mut nodes = Vec::new();

        for file_path in self.files.keys() {
            if let Some(parent) = file_path.parent()
                && parent == path
            {
                nodes.push(crate::NodeEntry::from_path(file_path.clone())?);
            }
        }

        for dir_path in &self.directories {
            if let Some(parent) = dir_path.parent()
                && parent == path
                && dir_path != &path.to_path_buf()
            {
                nodes.push(crate::NodeEntry::from_path(dir_path.clone())?);
            }
        }

        Ok(nodes)
    }

    async fn read(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| CoreError::not_found(path.to_path_buf()))
    }

    async fn read_range(
        &self,
        path: &Path,
        start: u64,
        len: u64,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        let content = self.read(path, cx).await?;
        let start = start as usize;
        let end = (start + len as usize).min(content.len());

        if start >= content.len() {
            return Ok(Vec::new());
        }

        Ok(content[start..end].to_vec())
    }

    async fn exists(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(self.files.contains_key(path) || self.directories.contains(&path.to_path_buf()))
    }

    async fn metadata(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<crate::NodeEntry, CoreError> {
        if self.files.contains_key(path) || self.directories.contains(&path.to_path_buf()) {
            Ok(crate::NodeEntry::from_path(path.to_path_buf())?)
        } else {
            Err(CoreError::not_found(path.to_path_buf()))
        }
    }
}

#[tokio::test]
async fn test_mock_fs_scheme() {
    let fs = MockFs::new();
    assert_eq!(fs.scheme(), "mock");
}

#[tokio::test]
async fn test_mock_fs_capabilities() {
    let fs = MockFs::new();
    let caps = fs.capabilities();

    assert!(caps.read);
    assert!(caps.write);
    assert!(!caps.watch);
    assert!(caps.search);
}

#[tokio::test]
async fn test_mock_fs_read() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");
    let content = b"Hello, MockFs!".to_vec();

    fs.add_file(path.clone(), content.clone());

    let result = fs.read(&path, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), content);
}

#[tokio::test]
async fn test_mock_fs_read_not_found() {
    let fs = MockFs::new();
    let result = fs
        .read(Path::new("/nonexistent.txt"), &crate::ProviderCx::none())
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
}

#[tokio::test]
async fn test_mock_fs_read_range() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");
    let content = b"0123456789".to_vec();

    fs.add_file(path.clone(), content);

    let result = fs.read_range(&path, 3, 4, &crate::ProviderCx::none()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"3456");
}

#[tokio::test]
async fn test_mock_fs_exists() {
    let mut fs = MockFs::new();
    let path = PathBuf::from("/test/file.txt");

    assert!(!fs.exists(&path, &crate::ProviderCx::none()).await.unwrap());

    fs.add_file(path.clone(), b"test".to_vec());
    assert!(fs.exists(&path, &crate::ProviderCx::none()).await.unwrap());
}

#[tokio::test]
async fn test_mock_fs_trait_usage() {
    // Test that MockFs can be used through the FsProvider trait
    let mut fs = MockFs::new();
    fs.add_file(PathBuf::from("/test.txt"), b"content".to_vec());

    let provider: &dyn FsProvider = &fs;
    assert_eq!(provider.scheme(), "mock");

    let result = provider
        .read(Path::new("/test.txt"), &crate::ProviderCx::none())
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"content");
}

mod write_tests {
    use super::*;

    fn fs() -> (LocalFs, TempDir) {
        local_fs()
    }

    #[tokio::test]
    async fn test_write_creates_new_file() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");

        fs.write(&path, b"hello", &crate::ProviderCx::none())
            .await
            .unwrap();

        let content = tokio::fs::read(&path).await.unwrap();
        assert_eq!(content, b"hello");
    }

    #[tokio::test]
    async fn test_write_overwrites_existing_file() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("file.txt");

        fs.write(&path, b"first", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.write(&path, b"second", &crate::ProviderCx::none())
            .await
            .unwrap();

        let content = tokio::fs::read(&path).await.unwrap();
        assert_eq!(content, b"second");
    }

    #[tokio::test]
    async fn test_write_returns_err_for_nonexistent_parent() {
        let (fs, dir) = local_fs();
        let path = dir.path().join("nonexistent").join("file.txt");

        let result = fs.write(&path, b"data", &crate::ProviderCx::none()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_copy_file_creates_destination() {
        let (fs, dir) = fs();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");

        fs.write(&src, b"data", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.copy(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert!(dst.exists());
    }

    #[tokio::test]
    async fn test_copy_file_preserves_content() {
        let (fs, dir) = fs();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");

        fs.write(&src, b"exact content", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.copy(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        let content = tokio::fs::read(&dst).await.unwrap();
        assert_eq!(content, b"exact content");
    }

    #[tokio::test]
    async fn test_copy_nonexistent_src_returns_err() {
        let (fs, dir) = fs();
        let src = dir.path().join("nonexistent.txt");
        let dst = dir.path().join("dst.txt");

        let result = fs.copy(&src, &dst, &crate::ProviderCx::none()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_file() {
        let (fs, dir) = fs();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");

        fs.write(&src, b"x", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.rename(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert!(dst.exists());
        assert!(!src.exists());
    }

    #[tokio::test]
    async fn test_rename_directory() {
        let (fs, dir) = fs();
        let src = dir.path().join("dir_a");
        let dst = dir.path().join("dir_b");

        tokio::fs::create_dir(&src).await.unwrap();
        fs.rename(&src, &dst, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert!(dst.exists());
        assert!(!src.exists());
    }

    #[tokio::test]
    async fn test_rename_nonexistent_returns_err() {
        let (fs, dir) = fs();
        let src = dir.path().join("nonexistent");
        let dst = dir.path().join("dst");

        let result = fs.rename(&src, &dst, &crate::ProviderCx::none()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_file() {
        let (fs, dir) = fs();
        let path = dir.path().join("file.txt");

        fs.write(&path, b"data", &crate::ProviderCx::none())
            .await
            .unwrap();
        fs.delete(&path, &crate::ProviderCx::none()).await.unwrap();

        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_delete_directory_recursively() {
        let (fs, dir) = fs();
        let root = dir.path().join("root");

        tokio::fs::create_dir(&root).await.unwrap();
        tokio::fs::create_dir(root.join("sub")).await.unwrap();
        tokio::fs::write(root.join("sub").join("file.txt"), b"data")
            .await
            .unwrap();

        fs.delete(&root, &crate::ProviderCx::none()).await.unwrap();

        assert!(!root.exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_err() {
        let (fs, dir) = fs();
        let path = dir.path().join("nonexistent.txt");

        let result = fs.delete(&path, &crate::ProviderCx::none()).await;
        assert_eq!(result.unwrap_err().code(), ErrorCode::PathNotFound);
    }

    #[tokio::test]
    async fn test_mkdir_creates_directory() {
        let (fs, dir) = fs();
        let path = dir.path().join("new_dir");

        fs.mkdir(&path, &crate::ProviderCx::none()).await.unwrap();

        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[tokio::test]
    async fn test_mkdir_creates_nested_directories() {
        let (fs, dir) = fs();
        let path = dir.path().join("a").join("b").join("c");

        fs.mkdir(&path, &crate::ProviderCx::none()).await.unwrap();

        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[tokio::test]
    async fn test_mkdir_existing_directory_is_ok() {
        let (fs, dir) = fs();
        let path = dir.path().join("existing");

        tokio::fs::create_dir(&path).await.unwrap();
        let result = fs.mkdir(&path, &crate::ProviderCx::none()).await;

        assert!(result.is_ok());
    }
}
