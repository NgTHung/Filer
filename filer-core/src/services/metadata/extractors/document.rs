use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::metadata::extended::{DocumentMetadata, ExtendedMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

/// Document metadata extractor.
///
/// PDF files use `lopdf::Document::load_metadata_from` for title, author,
/// page count, and dates. Office formats (DOCX/XLSX/PPTX/ODF/EPUB/RTF)
/// are not supported by `lopdf`; they return a stub with only the format
/// string populated — add a dedicated crate to fill those fields later.
pub struct DocumentExtractor;

impl DocumentExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract metadata from a PDF file via `lopdf`.
    #[cfg(feature = "metadata-document")]
    async fn extract_pdf(
        &self,
        path: &Path,
        provider: &dyn FsProvider,
    ) -> Result<DocumentMetadata, CoreError> {
        let reader = provider.open_reader(path).await?;
        let meta = lopdf::Document::load_metadata_from(reader)
            .map_err(|e| CoreError::InvalidData(format!("Cannot parse PDF: {e}")))?;

        Ok(DocumentMetadata {
            title: meta.title,
            author: meta.author,
            page_count: Some(meta.page_count),
            word_count: None, // requires full text extraction
            created: meta.creation_date,
            modified: meta.modification_date,
        })
    }
}

#[async_trait]
impl MetadataExtractor for DocumentExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Document]
    }

    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
    ) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-document"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-document")]
        {
            let meta = match &*mime.mime_type {
                "application/pdf" => self.extract_pdf(path, provider).await?,

                // Office / e-book formats: lopdf cannot read these.
                // Return a stub — add a dedicated crate (e.g. docx-rs) to fill fields.
                _ => DocumentMetadata {
                    title: None,
                    author: None,
                    page_count: None,
                    word_count: None,
                    created: None,
                    modified: None,
                },
            };

            Ok(ExtendedMetadata::Document(meta))
        }
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
