use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};

/// Archive metadata extractor (file count, compression ratio, entry listing)
pub struct ArchiveExtractor;

impl ArchiveExtractor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MetadataExtractor for ArchiveExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Archive]
    }

    async fn extract(&self, _path: &Path, _mime: &MimeInfo) -> Result<ExtendedMetadata, CoreError> {
        todo!()
    }

    fn name(&self) -> &'static str {
        "archive"
    }
}

impl Default for ArchiveExtractor {
    fn default() -> Self {
        Self::new()
    }
}
