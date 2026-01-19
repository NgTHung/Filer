use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::{AudioMetadata, AudioTags, ExtendedMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::MimeCategory;

/// Audio metadata extractor (duration, bitrate, tags)
pub struct AudioExtractor;

impl AudioExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract audio stream information
    async fn extract_stream_info(&self, path: &Path) -> Result<(f64, Option<u32>, Option<u8>, Option<u32>), CoreError> {
        todo!()
    }

    /// Extract audio tags (ID3, Vorbis, etc.)
    async fn extract_tags(&self, path: &Path) -> Result<AudioTags, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for AudioExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio]
    }

    fn supported_mime_types(&self) -> Option<&[&str]> {
        Some(&["audio/mpeg", "audio/flac", "audio/ogg", "audio/wav", "audio/aac", "audio/mp4"])
    }

    async fn extract(&self, path: &Path) -> Result<ExtendedMetadata, CoreError> {
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
