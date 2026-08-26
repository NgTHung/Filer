use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use crate::{
    ArchiveFs, Capabilities, CoreError, ErrorCode, FsProvider, NodeEntry, ProviderCx,
    ProviderProfile, ProviderProfileId, ProviderRef, ProviderRegistry,
};

fn profile() -> ProviderProfile {
    ProviderProfile::new(
        ProviderProfileId::new("work"),
        "archive",
        "Work archive",
        PathBuf::from("/archives/work.zip"),
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        },
    )
}

#[test]
fn provider_profile_round_trips_without_secrets() {
    let encoded = serde_json::to_string(&profile()).unwrap();
    let debug = format!("{:?}", profile());

    assert!(!encoded.contains("credential"));
    assert!(!encoded.contains("password"));
    assert!(!encoded.contains("secret"));
    assert!(!encoded.contains("token"));
    assert!(!debug.contains("credential"));
    assert!(!debug.contains("password"));
    assert!(!debug.contains("secret"));
    assert!(!debug.contains("token"));

    let decoded: ProviderProfile = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, profile());
    assert_eq!(decoded.id().as_str(), "work");
    assert_eq!(decoded.scheme(), "archive");
    assert_eq!(decoded.display_name(), "Work archive");
    assert_eq!(decoded.default_root(), PathBuf::from("/archives/work.zip"));
}

#[test]
fn provider_capabilities_are_serializable_contracts() {
    let capabilities = Capabilities {
        read: true,
        write: false,
        watch: true,
        search: false,
    };

    let encoded = serde_json::to_string(&capabilities).unwrap();
    let decoded: Capabilities = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, capabilities);
}

struct NamedProvider {
    scheme: &'static str,
    capabilities: Capabilities,
}

impl NamedProvider {
    fn new(scheme: &'static str) -> Self {
        Self {
            scheme,
            capabilities: Capabilities {
                read: true,
                write: false,
                watch: false,
                search: false,
            },
        }
    }
}

#[async_trait]
impl FsProvider for NamedProvider {
    fn scheme(&self) -> &'static str {
        self.scheme
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    async fn list(
        &self,
        _path: &std::path::Path,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<NodeEntry>, CoreError> {
        Ok(Vec::new())
    }

    async fn read(
        &self,
        path: &std::path::Path,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::unsupported_operation(format!(
            "read unsupported: {}",
            path.display()
        )))
    }

    async fn read_range(
        &self,
        path: &std::path::Path,
        _start: u64,
        _len: u64,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::unsupported_operation(format!(
            "read range unsupported: {}",
            path.display()
        )))
    }

    async fn exists(
        &self,
        _path: &std::path::Path,
        _cx: &ProviderCx<'_>,
    ) -> Result<bool, CoreError> {
        Ok(false)
    }

    async fn metadata(
        &self,
        path: &std::path::Path,
        _cx: &ProviderCx<'_>,
    ) -> Result<NodeEntry, CoreError> {
        Ok(NodeEntry::from_path(path.to_path_buf())?)
    }
}

#[test]
fn provider_registry_resolves_local_profile_and_ephemeral_refs() {
    let local = Arc::new(NamedProvider::new("file"));
    let registry = ProviderRegistry::new(local.clone());
    let profile_provider = Arc::new(NamedProvider::new("archive"));
    let ephemeral_provider = Arc::new(NamedProvider::new("memory"));
    let profile = ProviderProfile::new(
        ProviderProfileId::new("assets"),
        "archive",
        "Assets",
        PathBuf::from("/archives/assets.zip"),
        profile_provider.capabilities(),
    );

    registry
        .register_profile(profile, profile_provider.clone())
        .unwrap();
    registry.register_ephemeral("session", ephemeral_provider.clone());

    assert!(Arc::ptr_eq(
        &registry.resolve(&ProviderRef::Local).unwrap(),
        &(local as Arc<dyn FsProvider>)
    ));
    assert!(Arc::ptr_eq(
        &registry
            .resolve(&ProviderRef::Profile("assets".to_string()))
            .unwrap(),
        &(profile_provider as Arc<dyn FsProvider>)
    ));
    assert!(Arc::ptr_eq(
        &registry
            .resolve(&ProviderRef::Ephemeral("session".to_string()))
            .unwrap(),
        &(ephemeral_provider as Arc<dyn FsProvider>)
    ));
}

#[test]
fn provider_registry_rejects_profile_scheme_mismatch() {
    let registry = ProviderRegistry::new(Arc::new(NamedProvider::new("file")));
    let provider = Arc::new(NamedProvider::new("archive"));
    let profile = ProviderProfile::new(
        ProviderProfileId::new("assets"),
        "sftp",
        "Assets",
        PathBuf::from("/archives/assets.zip"),
        provider.capabilities(),
    );

    let error = registry.register_profile(profile, provider).unwrap_err();

    assert_eq!(error.code(), ErrorCode::InputInvalid);
}

#[test]
fn provider_registry_reports_unknown_refs_as_unsupported_provider() {
    let registry = ProviderRegistry::new(Arc::new(NamedProvider::new("file")));

    let profile = match registry.resolve(&ProviderRef::Profile("missing".to_string())) {
        Ok(_) => panic!("missing profile should not resolve"),
        Err(error) => error,
    };
    let ephemeral = match registry.resolve(&ProviderRef::Ephemeral("missing".to_string())) {
        Ok(_) => panic!("missing ephemeral provider should not resolve"),
        Err(error) => error,
    };

    assert_eq!(profile.code(), ErrorCode::UnsupportedProvider);
    assert_eq!(ephemeral.code(), ErrorCode::UnsupportedProvider);
}

#[cfg(feature = "metadata-archive")]
mod archive_provider_tests {
    use super::*;
    use crate::LocalFs;
    use crate::vfs::provider::ReadSeek;
    use crate::{LocationDescriptor, SegmentedLocationResolver};
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn archive_fs(path: PathBuf) -> (ArchiveFs, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let provider = Arc::new(LocalFs::new());
        (ArchiveFs::zip(path, provider), dir)
    }

    #[tokio::test]
    async fn archive_fs_lists_zip_root_children() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bundle.zip");
        write_zip(
            &archive,
            &[("src/main.rs", b"fn main() {}"), ("README.md", b"readme")],
        );
        let provider = Arc::new(LocalFs::new());
        let fs = ArchiveFs::zip(archive.clone(), provider);

        let entries = fs
            .list(std::path::Path::new(""), &ProviderCx::none())
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        let src = entries.iter().find(|entry| entry.name == "src").unwrap();
        assert!(src.is_dir());
        assert_eq!(
            src.location.descriptor(),
            Some(&crate::LocationDescriptor::local(&archive).archive_member("src"))
        );
        let readme = entries
            .iter()
            .find(|entry| entry.name == "README.md")
            .unwrap();
        assert!(readme.is_file());
        assert_eq!(readme.size, 6);
        assert_eq!(
            readme.location.descriptor(),
            Some(&crate::LocationDescriptor::local(&archive).archive_member("README.md"))
        );
    }

    #[tokio::test]
    async fn archive_fs_lists_nested_zip_directory_children() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bundle.zip");
        write_zip(
            &archive,
            &[
                ("src/main.rs", b"fn main() {}"),
                ("src/lib.rs", b"pub fn lib() {}"),
                ("README.md", b"readme"),
            ],
        );
        let provider = Arc::new(LocalFs::new());
        let fs = ArchiveFs::zip(archive.clone(), provider);

        let entries = fs
            .list(std::path::Path::new("src"), &ProviderCx::none())
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.name == "main.rs"));
        assert!(entries.iter().any(|entry| entry.name == "lib.rs"));
        for entry in entries {
            assert_eq!(
                entry.location.descriptor(),
                Some(
                    &crate::LocationDescriptor::local(&archive)
                        .archive_member(format!("src/{}", entry.name),)
                )
            );
        }
    }

    #[tokio::test]
    async fn archive_fs_reports_read_only_capabilities() {
        let (fs, _dir) = archive_fs(PathBuf::from("unused.zip"));
        let caps = fs.capabilities();

        assert!(caps.read);
        assert!(!caps.write);
        assert!(!caps.watch);
        assert!(!caps.search);
    }

    #[tokio::test]
    async fn archive_fs_member_reads_are_out_of_scope() {
        let (fs, _dir) = archive_fs(PathBuf::from("unused.zip"));

        let error = fs
            .read(std::path::Path::new("README.md"), &ProviderCx::none())
            .await
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::UnsupportedOperation);
    }

    struct OpenReaderCountingProvider {
        inner: LocalFs,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl FsProvider for OpenReaderCountingProvider {
        fn scheme(&self) -> &'static str {
            self.inner.scheme()
        }

        fn capabilities(&self) -> Capabilities {
            self.inner.capabilities()
        }

        async fn list(
            &self,
            path: &std::path::Path,
            cx: &ProviderCx<'_>,
        ) -> Result<Vec<NodeEntry>, CoreError> {
            self.inner.list(path, cx).await
        }

        async fn read(
            &self,
            path: &std::path::Path,
            cx: &ProviderCx<'_>,
        ) -> Result<Vec<u8>, CoreError> {
            self.inner.read(path, cx).await
        }

        async fn read_range(
            &self,
            path: &std::path::Path,
            start: u64,
            len: u64,
            cx: &ProviderCx<'_>,
        ) -> Result<Vec<u8>, CoreError> {
            self.inner.read_range(path, start, len, cx).await
        }

        async fn exists(
            &self,
            path: &std::path::Path,
            cx: &ProviderCx<'_>,
        ) -> Result<bool, CoreError> {
            self.inner.exists(path, cx).await
        }

        async fn metadata(
            &self,
            path: &std::path::Path,
            cx: &ProviderCx<'_>,
        ) -> Result<NodeEntry, CoreError> {
            self.inner.metadata(path, cx).await
        }

        async fn open_reader(
            &self,
            path: &std::path::Path,
            cx: &ProviderCx<'_>,
        ) -> Result<Box<dyn ReadSeek>, CoreError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.inner.open_reader(path, cx).await
        }
    }

    #[tokio::test]
    async fn segmented_archive_listing_uses_provider_open_reader() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bundle.zip");
        write_zip(&archive, &[("src/main.rs", b"fn main() {}")]);
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = OpenReaderCountingProvider {
            inner: LocalFs::new(),
            calls: calls.clone(),
        };
        let location = LocationDescriptor::local(&archive).archive_member("");

        let entries = SegmentedLocationResolver::new(&provider)
            .list(&location, &ProviderCx::none())
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "src");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
