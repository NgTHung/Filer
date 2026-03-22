use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

#[cfg(feature = "metadata-image")]
use crate::services::metadata::extended::{ExifData, ImageMetadata};

/// Image metadata extractor (dimensions, format, EXIF)
pub struct ImageExtractor;

impl ImageExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract basic image dimensions and format
    #[cfg(feature = "metadata-image")]
    async fn extract_dimensions(
        &self,
        _path: &Path,
        _provider: &dyn FsProvider,
    ) -> Result<(u32, u32, String), CoreError> {
        todo!()
    }

    /// Extract EXIF data from image
    #[cfg(feature = "metadata-image")]
    async fn extract_exif(
        &self,
        _path: &Path,
        _provider: &dyn FsProvider,
    ) -> Result<Option<ExifData>, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for ImageExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Image]
    }

    async fn extract(
        &self,
        _path: &Path,
        _mime: &MimeInfo,
        _provider: &dyn FsProvider,
    ) -> Result<ExtendedMetadata, CoreError> {
        #[cfg(not(feature = "metadata-image"))]
        return Ok(ExtendedMetadata::Unavailable);

        #[cfg(feature = "metadata-image")]
        todo!()
    }

    fn name(&self) -> &'static str {
        "image"
    }
}

impl Default for ImageExtractor {
    fn default() -> Self {
        Self::new()
    }
}
