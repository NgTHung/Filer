use std::path::Path;

use crate::errors::CoreError;
use crate::services::mime::{MimeDetector, MimeInfo};

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
    mime_detector: MimeDetector,
    default_options: PreviewOptions,
}

impl PreviewRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            mime_detector: MimeDetector::new(),
            default_options: PreviewOptions::default(),
        }
    }

    /// Create registry with all built-in providers pre-registered.
    pub fn with_defaults() -> Self {
        todo!()
    }

    /// Register a preview provider.
    ///
    /// Providers are sorted by `priority()` after insertion so that
    /// `get_provider` always returns the highest-priority match.
    pub fn register(&mut self, provider: Box<dyn PreviewProvider>) {
        self.providers.push(provider);
        self.providers.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    /// Set default preview options.
    pub fn set_default_options(&mut self, options: PreviewOptions) {
        self.default_options = options;
    }

    /// Generate preview for a file using default options.
    pub async fn generate(&self, path: &Path) -> Result<PreviewData, CoreError> {
        self.generate_with_options(path, &self.default_options).await
    }

    /// Generate preview using tiered MIME detection then provider dispatch.
    ///
    /// ## Detection tiers
    ///
    /// ### Tier 1 — Extension (zero I/O)
    /// TODO: Call `self.mime_detector.detect_from_path(path)`.
    /// Skip Tier 2 when `confidence == Definitive` and strategy is not
    /// `MagicBytes`, or when strategy is `ExtensionOnly`.
    ///
    /// ### Tier 2 — Magic bytes (512-byte read)
    /// TODO: Call `provider.read_header(path, 512)`:
    /// - `Ok(bytes)` → `self.mime_detector.detect_with_strategy(path, &bytes, strategy)`
    /// - `Err(_)`    → remote/unreadable, fall back to Tier 1 silently.
    /// Magic bytes win when they disagree with the extension.
    ///
    /// ### Tier 3 — Deep content (inside providers)
    /// TODO: Text providers read further bytes to distinguish CSV / JSON / TOML
    /// when the sub-format affects rendering.
    ///
    /// ## Dispatch
    /// TODO: Call `self.get_provider(&mime)`.
    /// - `Some(p)` → `p.generate(path, &mime, options).await`
    /// - `None`    → `PreviewData::Unsupported { mime_type, reason }`
    pub async fn generate_with_options(
        &self,
        _path: &Path,
        _options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        todo!()
    }

    /// Check if a preview provider is registered for this path's MIME type.
    pub fn can_preview(&self, _path: &Path) -> bool {
        todo!()
    }

    /// Get the highest-priority provider that handles `mime.category`.
    fn get_provider(&self, mime: &MimeInfo) -> Option<&dyn PreviewProvider> {
        self.providers
            .iter()
            .find(|p| p.supported_categories().contains(&mime.category))
            .map(|p| p.as_ref())
    }
}

impl Default for PreviewRegistry {
    fn default() -> Self {
        Self::new()
    }
}
