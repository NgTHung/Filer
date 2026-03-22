use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

#[cfg(feature = "metadata-video")]
use crate::services::metadata::extended::VideoMetadata;

/// Video metadata extractor (dimensions, duration, codecs)
pub struct VideoExtractor;

impl VideoExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract video stream information
    #[cfg(feature = "metadata-video")]
    async fn extract_video_stream(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<(u32, u32, Option<f32>, Option<String>), CoreError> {
        todo!()
    }

    /// Extract audio stream information from video
    #[cfg(feature = "metadata-video")]
    async fn extract_audio_stream(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<Option<String>, CoreError> {
        todo!()
    }

    /// Extract duration
    #[cfg(feature = "metadata-video")]
    async fn extract_duration(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<f64, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for VideoExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Video]
    }

    async fn extract(&self, _path: &Path, _mime: &MimeInfo, _provider: &dyn FsProvider) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-video"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-video")]
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
