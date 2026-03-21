use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::{ExtendedMetadata, VideoMetadata};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};

/// Video metadata extractor (dimensions, duration, codecs)
pub struct VideoExtractor;

impl VideoExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract video stream information
    async fn extract_video_stream(&self, _path: &Path) -> Result<(u32, u32, Option<f32>, Option<String>), CoreError> {
        todo!()
    }

    /// Extract audio stream information from video
    async fn extract_audio_stream(&self, _path: &Path) -> Result<Option<String>, CoreError> {
        todo!()
    }

    /// Extract duration
    async fn extract_duration(&self, _path: &Path) -> Result<f64, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for VideoExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Video]
    }

    async fn extract(&self, path: &Path, _mime: &MimeInfo) -> Result<ExtendedMetadata, CoreError> {
        todo!()
    }

    fn name(&self) -> &'static str {
        "video"
    }
}

impl Default for VideoExtractor {
    fn default() -> Self {
        Self::new()
    }
}
