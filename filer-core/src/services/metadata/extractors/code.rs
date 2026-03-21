use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};

/// Code/text metadata extractor (language, line count)
pub struct CodeExtractor;

impl CodeExtractor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MetadataExtractor for CodeExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    async fn extract(&self, _path: &Path, _mime: &MimeInfo) -> Result<ExtendedMetadata, CoreError> {
        todo!()
    }

    fn name(&self) -> &'static str {
        "code"
    }
}

impl Default for CodeExtractor {
    fn default() -> Self {
        Self::new()
    }
}
