use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::{DocumentMetadata, ExtendedMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};

/// Document metadata extractor (PDF, Office documents)
pub struct DocumentExtractor;

impl DocumentExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract PDF metadata
    async fn extract_pdf(&self, _path: &Path) -> Result<DocumentMetadata, CoreError> {
        todo!()
    }

    /// Extract Office document metadata (docx, xlsx, etc.)
    async fn extract_office(&self, _path: &Path) -> Result<DocumentMetadata, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for DocumentExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Document]
    }

    async fn extract(&self, path: &Path, _mime: &MimeInfo) -> Result<ExtendedMetadata, CoreError> {
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
