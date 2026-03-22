use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

#[cfg(feature = "metadata-audio")]
use crate::services::metadata::extended::AudioTags;

/// Audio metadata extractor (duration, bitrate, tags)
pub struct AudioExtractor;

impl AudioExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract audio stream information
    #[cfg(feature = "metadata-audio")]
    async fn extract_stream_info(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<(f64, Option<u32>, Option<u8>, Option<u32>), CoreError> {
        todo!()
    }

    /// Extract audio tags (ID3, Vorbis, etc.)
    #[cfg(feature = "metadata-audio")]
    async fn extract_tags(&self, _path: &Path, _provider: &dyn FsProvider) -> Result<AudioTags, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for AudioExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio]
    }

    async fn extract(&self, _path: &Path, _mime: &MimeInfo, _provider: &dyn FsProvider) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-audio"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-audio")]
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
