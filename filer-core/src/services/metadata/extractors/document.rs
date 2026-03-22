use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

#[cfg(feature = "metadata-document")]
use crate::services::metadata::extended::DocumentMetadata;

/// Document metadata extractor (PDF, Office documents)
pub struct DocumentExtractor;

impl DocumentExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract PDF metadata
    #[cfg(feature = "metadata-document")]
    async fn extract_pdf(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<DocumentMetadata, CoreError> {
        todo!()
    }

    /// Extract Office document metadata (docx, xlsx, etc.)
    #[cfg(feature = "metadata-document")]
    async fn extract_office(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<DocumentMetadata, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for DocumentExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Document]
    }

    async fn extract(&self, _path: &Path, _mime: &MimeInfo, _provider: &dyn FsProvider) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-document"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-document")]
        todo!()
    }

    fn name(&self) -> &'static str {
        "document"
    }
}

impl Default for DocumentExtractor {
    fn default() -> Self {
        Self::new()
    }
}
