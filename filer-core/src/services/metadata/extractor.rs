use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

use super::extended::ExtendedMetadata;

/// Trait for type-specific metadata extractors.
///
/// The registry routes by `MimeCategory`, so each extractor receives a
/// fully-detected `MimeInfo` and never needs to re-detect the file type.
/// Intra-category branching (e.g. JPEG vs PNG EXIF parsing) is done
/// inside `extract` by inspecting `mime.mime_type`.
#[async_trait]
pub trait MetadataExtractor: Send + Sync {
    /// Broad categories this extractor handles (used for registry routing).
    fn supported_categories(&self) -> &[MimeCategory];

    /// Extract metadata from the file at `path`.
    ///
    /// `mime` carries the already-detected MIME information. Branch on
    /// `mime.mime_type` for format-specific logic and use `mime.confidence`
    /// to decide whether expensive extraction is warranted when the type is
    /// uncertain.
    ///
    /// `provider` is used for all I/O so that remote backends (S3, SFTP,
    /// WebDAV) work without the extractors having any direct filesystem
    /// dependency. Use `provider.read(path).await` to fetch content, or
    /// `provider.read_range()` for large files where a partial read suffices.
    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
    ) -> Result<ExtendedMetadata, CoreError>;

    /// Extractor name for logging and debugging.
    fn name(&self) -> &'static str;
}

/// Registry that routes `MimeInfo` to the right `MetadataExtractor`.
///
/// Routing:
///   1. Find the first registered extractor whose `supported_categories`
///      contains `mime.category`.
///   2. No match → `Ok(ExtendedMetadata::Unavailable)`.
///
/// Build with `register` calls (takes `&mut self`), then wrap in `Arc` for
/// sharing across tasks. The registry is intentionally read-only after
/// construction — concurrent access requires no locking.
pub struct MetadataRegistry {
    extractors: Vec<Box<dyn MetadataExtractor>>,
}

impl MetadataRegistry {
    pub fn new() -> Self {
        Self {
            extractors: Vec::new(),
        }
    }

    /// Create a registry with all built-in extractors pre-registered.
    pub fn with_defaults() -> Self {
        use super::extractors::{
            ArchiveExtractor, AudioExtractor, CodeExtractor, DocumentExtractor, ImageExtractor,
            VideoExtractor,
        };
        let mut reg = Self::new();
        reg.register(Box::new(ImageExtractor::new()));
        reg.register(Box::new(AudioExtractor::new()));
        reg.register(Box::new(VideoExtractor::new()));
        reg.register(Box::new(DocumentExtractor::new()));
        reg.register(Box::new(ArchiveExtractor::new()));
        reg.register(Box::new(CodeExtractor::new()));
        reg
    }

    /// Register an extractor.
    ///
    /// Extractors are stored in registration order. When multiple extractors
    /// share a category, the first registered wins. Register more-specific
    /// extractors before more-general ones.
    pub fn register(&mut self, extractor: Box<dyn MetadataExtractor>) {
        self.extractors.push(extractor);
    }

    /// Find the best extractor for `mime`.
    ///
    /// Returns the first registered extractor whose `supported_categories`
    /// includes `mime.category`, or `None` when no extractor covers this type.
    pub fn get(&self, mime: &MimeInfo) -> Option<&dyn MetadataExtractor> {
        self.extractors
            .iter()
            .find(|e| e.supported_categories().contains(&mime.category))
            .map(|e| e.as_ref())
    }

    /// Extract metadata for `path` using the best extractor for `mime`.
    ///
    /// Returns `Ok(ExtendedMetadata::Unavailable)` when no extractor is
    /// registered for `mime.category`.
    pub async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
    ) -> Result<ExtendedMetadata, CoreError> {
        match self.get(mime) {
            Some(extractor) => extractor.extract(path, mime, provider).await,
            None => Ok(ExtendedMetadata::Unavailable),
        }
    }
}

impl Default for MetadataRegistry {
    fn default() -> Self {
        Self::new()
    }
}
