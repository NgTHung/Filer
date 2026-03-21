use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::{AudioTags, ExtendedMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};

/// Audio metadata extractor (duration, bitrate, tags)
pub struct AudioExtractor;

impl AudioExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract audio stream information
    async fn extract_stream_info(&self, _path: &Path) -> Result<(f64, Option<u32>, Option<u8>, Option<u32>), CoreError> {
        todo!()
    }

    /// Extract audio tags (ID3, Vorbis, etc.)
    async fn extract_tags(&self, _path: &Path) -> Result<AudioTags, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for AudioExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio]
    }

    async fn extract(&self, path: &Path, _mime: &MimeInfo) -> Result<ExtendedMetadata, CoreError> {
        todo!()
    }

    fn name(&self) -> &'static str {
        "audio"
    }
}

impl Default for AudioExtractor {
    fn default() -> Self {
        Self::new()
    }
}
