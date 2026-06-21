use std::path::Path;

use crate::errors::CoreError;
use crate::services::mime::{
    DetectionConfidence, DetectionStrategy, MAGIC_BYTE_WINDOW, MimeDetector, MimeInfo,
};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

use super::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Registry of preview providers.
///
/// Routing:
///   1. Find the highest-priority registered provider whose `supported_categories`
///      contains `mime.category`.
///   2. No match → `PreviewData::Unsupported { mime_type, reason }`.
///
/// Build with `register` calls (takes `&mut self`), then wrap in `Arc` for
/// sharing. The registry is read-only after construction.
pub struct PreviewRegistry {
    /// Stored in descending priority order (highest first).
    providers: Vec<Box<dyn PreviewProvider>>,
    default_options: PreviewOptions,
}

impl PreviewRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            default_options: PreviewOptions::default(),
        }
    }

    /// Register a preview provider.
    ///
    /// Providers are sorted by `priority()` after insertion so that
    /// `get_provider` always returns the highest-priority match.
    pub fn register(&mut self, provider: Box<dyn PreviewProvider>) {
        self.providers.push(provider);
        self.providers
            .sort_by_key(|b| std::cmp::Reverse(b.priority()))
    }

    /// Set default preview options.
    pub fn set_default_options(&mut self, options: PreviewOptions) {
        self.default_options = options;
    }

    /// Generate preview for a file using default options.
    pub async fn generate(
        &self,
        path: &Path,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<PreviewData, CoreError> {
        self.generate_with_options(path, &self.default_options, provider, cx)
            .await
    }

    /// Create registry with all built-in providers pre-registered.
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Generate preview using tiered MIME detection then provider dispatch.
    ///
    /// **Tier 1** — Extension (zero I/O): `MimeDetector::detect_from_path`.
    /// Returns immediately when `confidence == Definitive` and strategy is not
    /// `MagicBytes` — this correctly keeps `.docx` as Document instead of
    /// being overridden by its ZIP magic bytes.
    ///
    /// **Tier 2** — Magic bytes (reads the file head through the provider).
    /// Skipped when strategy is `ExtensionOnly` or Tier 1 was Definitive.
    /// On I/O failure the read is silently dropped and Tier 1 result is used.
    ///
    /// **Tier 3** — Deep content: handled inside individual providers (e.g.
    /// `TextProvider` reads further bytes to distinguish CSV/JSON/TOML).
    pub async fn generate_with_options(
        &self,
        path: &Path,
        options: &PreviewOptions,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> Result<PreviewData, CoreError> {
        let mime = self.detect_mime(path, options, provider, cx).await;
        match self.get_provider(&mime) {
            Some(p) => p.generate(path, &mime, options).await,
            None => Ok(PreviewData::Unsupported {
                mime_type: mime.mime_type,
                reason: "No preview provider registered for this file type".to_string(),
            }),
        }
    }

    /// Check if any registered provider can handle this path's MIME type.
    /// Uses extension-only detection (zero I/O).
    pub fn can_preview(&self, path: &Path) -> bool {
        let mime = MimeDetector::detect_from_path(path);
        self.get_provider(&mime).is_some()
    }

    /// Run the two-tier MIME detection pipeline for `path`.
    async fn detect_mime(
        &self,
        path: &Path,
        options: &PreviewOptions,
        provider: &dyn FsProvider,
        cx: &ProviderCx<'_>,
    ) -> MimeInfo {
        // Tier 1 — extension
        let ext_info = MimeDetector::detect_from_path(path);

        // Early return: Definitive result + strategy does not force magic bytes.
        if ext_info.confidence == DetectionConfidence::Definitive
            && options.detection_strategy != DetectionStrategy::MagicBytes
        {
            return ext_info;
        }
        // Early return: caller explicitly opted out of any I/O.
        if options.detection_strategy == DetectionStrategy::ExtensionOnly {
            return ext_info;
        }

        // Tier 2 — magic bytes. The provider read keeps detection consistent
        // with the rest of the VFS; failures fall back to the Tier 1 result.
        let header = provider
            .read_header(path, MAGIC_BYTE_WINDOW, cx)
            .await
            .ok();
        MimeDetector::detect_with_strategy(path, header.as_deref(), options.detection_strategy)
    }

    /// Get the highest-priority provider that handles `mime.category`.
    pub fn get_provider_pub(&self, mime: &MimeInfo) -> Option<&dyn PreviewProvider> {
        self.get_provider(mime)
    }

    fn get_provider(&self, mime: &MimeInfo) -> Option<&dyn PreviewProvider> {
        self.providers
            .iter()
            .find(|p| p.supported_categories().contains(&mime.category))
            .map(|p| p.as_ref())
    }
}

impl Default for PreviewRegistry {
    fn default() -> Self {
        use super::providers::*;
        let mut reg = Self::new();
        #[cfg(feature = "preview-code")]
        reg.register(Box::new(CodeProvider::new()));
        reg.register(Box::new(ImageProvider::new()));
        reg.register(Box::new(MediaProvider::new()));
        reg.register(Box::new(ArchiveProvider::new()));
        reg.register(Box::new(TextProvider::new()));
        reg
    }
}
