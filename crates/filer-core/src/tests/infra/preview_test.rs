use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{DetectionConfidence, DetectionStrategy, MimeCategory, MimeInfo};
use crate::services::preview::{
    PreviewCache, PreviewData, PreviewOptions, PreviewProvider, PreviewRegistry,
};
use crate::vfs::provider::{Capabilities, FsProvider};

fn text_preview(content: &str) -> PreviewData {
    PreviewData::Text {
        content: content.to_string(),
        truncated: false,
        total_lines: content.lines().count(),
    }
}

fn mime(category: MimeCategory) -> MimeInfo {
    MimeInfo {
        mime_type: "application/octet-stream".to_string(),
        category,
        encoding: None,
        confidence: DetectionConfidence::Definitive,
    }
}

struct StubProvider {
    categories: &'static [MimeCategory],
    priority: u8,
    name: &'static str,
}

#[async_trait]
impl PreviewProvider for StubProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        self.categories
    }
    async fn generate(
        &self,
        _path: &std::path::Path,
        _mime: &MimeInfo,
        _options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        Ok(text_preview(self.name))
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

struct HeaderRecordingProvider {
    saw_cancel: Arc<Mutex<bool>>,
}

#[async_trait]
impl FsProvider for HeaderRecordingProvider {
    fn scheme(&self) -> &'static str {
        "recording"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    async fn list(
        &self,
        _: &Path,
        _: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        Ok(Vec::new())
    }

    async fn read(&self, path: &Path, _: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }

    async fn read_range(
        &self,
        path: &Path,
        _: u64,
        _: u64,
        _: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }

    async fn read_header(
        &self,
        _: &Path,
        _: usize,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        *self.saw_cancel.lock().unwrap() = cx.cancel.is_some();
        Ok(b"hello".to_vec())
    }

    async fn exists(&self, _: &Path, _: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        path: &Path,
        _: &crate::ProviderCx<'_>,
    ) -> Result<crate::NodeEntry, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn test_get_provider_returns_highest_priority_for_text() {
        let mut reg = PreviewRegistry::new();
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 50,
            name: "low",
        }));
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 100,
            name: "high",
        }));

        let info = mime(MimeCategory::Text);
        let provider = reg.get_provider_pub(&info).unwrap();
        assert_eq!(provider.name(), "high");
    }

    #[test]
    fn test_priority_ordering_on_register() {
        let mut reg = PreviewRegistry::new();
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 10,
            name: "last",
        }));
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 200,
            name: "first",
        }));
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 100,
            name: "middle",
        }));

        let info = mime(MimeCategory::Text);
        assert_eq!(reg.get_provider_pub(&info).unwrap().name(), "first");
    }

    #[test]
    fn test_get_provider_returns_correct_category() {
        let mut reg = PreviewRegistry::new();
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Image],
            priority: 100,
            name: "image",
        }));
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Audio],
            priority: 100,
            name: "audio",
        }));
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 100,
            name: "text",
        }));

        assert_eq!(
            reg.get_provider_pub(&mime(MimeCategory::Image))
                .unwrap()
                .name(),
            "image"
        );
        assert_eq!(
            reg.get_provider_pub(&mime(MimeCategory::Audio))
                .unwrap()
                .name(),
            "audio"
        );
        assert_eq!(
            reg.get_provider_pub(&mime(MimeCategory::Text))
                .unwrap()
                .name(),
            "text"
        );
    }

    #[test]
    fn test_can_preview_returns_false_for_unsupported() {
        let reg = PreviewRegistry::new();
        assert!(!reg.can_preview(std::path::Path::new("file.xyz_unknown_ext")));
    }

    #[test]
    fn test_can_preview_returns_true_when_provider_exists() {
        let mut reg = PreviewRegistry::new();
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 100,
            name: "text",
        }));
        assert!(reg.can_preview(std::path::Path::new("file.txt")));
    }

    #[test]
    fn test_get_provider_returns_none_for_unregistered_category() {
        let mut reg = PreviewRegistry::new();
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Image],
            priority: 100,
            name: "image",
        }));
        assert!(reg.get_provider_pub(&mime(MimeCategory::Audio)).is_none());
    }

    #[tokio::test]
    async fn generate_with_options_passes_context_to_mime_header_read() {
        let mut reg = PreviewRegistry::new();
        reg.register(Box::new(StubProvider {
            categories: &[MimeCategory::Text],
            priority: 100,
            name: "text",
        }));
        let saw_cancel = Arc::new(Mutex::new(false));
        let provider = HeaderRecordingProvider {
            saw_cancel: saw_cancel.clone(),
        };
        let cancel = crate::CancelSignal::new();
        let cx = crate::ProviderCx::with_cancel(&cancel);
        let options = PreviewOptions {
            detection_strategy: DetectionStrategy::MagicBytes,
            ..PreviewOptions::default()
        };

        let result = reg
            .generate_with_options(Path::new("ambiguous.txt"), &options, &provider, &cx)
            .await
            .unwrap();

        assert!(matches!(result, PreviewData::Text { .. }));
        assert!(
            *saw_cancel.lock().unwrap(),
            "MIME fallback read must receive the preview ProviderCx"
        );
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn make_cache(max_bytes: usize, ttl: Duration) -> PreviewCache {
        PreviewCache::new(max_bytes, ttl)
    }

    #[test]
    fn test_cache_miss_returns_none() {
        let cache = make_cache(1024 * 1024, Duration::from_secs(60));
        assert!(cache.get(&PathBuf::from("/some/path")).is_none());
    }

    #[test]
    fn test_cache_hit_returns_clone() {
        let mut cache = make_cache(1024 * 1024, Duration::from_secs(60));
        let path = PathBuf::from("/tmp/test.txt");
        let preview = text_preview("hello world");
        cache.put(path.clone(), preview);

        let result = cache.get(&path).unwrap();
        assert!(matches!(result, PreviewData::Text { .. }));
    }

    #[test]
    fn test_ttl_expiry_returns_none() {
        let mut cache = make_cache(1024 * 1024, Duration::from_millis(1));
        let path = PathBuf::from("/tmp/test.txt");
        cache.put(path.clone(), text_preview("data"));

        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get(&path).is_none());
    }

    #[test]
    fn test_invalidate_removes_entry() {
        let mut cache = make_cache(1024 * 1024, Duration::from_secs(60));
        let path = PathBuf::from("/tmp/test.txt");
        cache.put(path.clone(), text_preview("data"));
        cache.invalidate(&path);
        assert!(cache.get(&path).is_none());
    }

    #[test]
    fn test_clear_empties_cache() {
        let mut cache = make_cache(1024 * 1024, Duration::from_secs(60));
        let p1 = PathBuf::from("/tmp/a.txt");
        let p2 = PathBuf::from("/tmp/b.txt");
        cache.put(p1.clone(), text_preview("a"));
        cache.put(p2.clone(), text_preview("b"));
        cache.clear();
        assert!(cache.get(&p1).is_none());
        assert!(cache.get(&p2).is_none());
    }

    #[test]
    fn test_size_eviction_clears_when_over_capacity() {
        // max_bytes = 5, each entry ~ content.len() bytes
        let mut cache = make_cache(5, Duration::from_secs(60));
        let p1 = PathBuf::from("/tmp/a.txt");
        let p2 = PathBuf::from("/tmp/b.txt");

        cache.put(p1.clone(), text_preview("abc")); // 3 bytes — fits
        // Adding "defgh" (5 bytes) would exceed capacity; nuclear eviction clears everything
        cache.put(p2.clone(), text_preview("defgh"));

        // After nuclear eviction, p1 is gone; p2 was just inserted
        assert!(cache.get(&p1).is_none());
        assert!(cache.get(&p2).is_some());
    }

    #[test]
    fn test_put_replaces_existing_entry() {
        let mut cache = make_cache(1024 * 1024, Duration::from_secs(60));
        let path = PathBuf::from("/tmp/test.txt");
        cache.put(path.clone(), text_preview("old"));
        cache.put(path.clone(), text_preview("new"));

        match cache.get(&path).unwrap() {
            PreviewData::Text { content, .. } => assert_eq!(content, "new"),
            _ => panic!("unexpected variant"),
        }
    }
}
