use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::metadata::extended::{ExtendedMetadata, ImageMetadata, ExifData};
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::MimeCategory;

/// Image metadata extractor (dimensions, format, EXIF)
pub struct ImageExtractor;

impl ImageExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract basic image dimensions and format
    async fn extract_dimensions(&self, path: &Path) -> Result<(u32, u32, String), CoreError> {
        todo!()
    }

    /// Extract EXIF data from image
    async fn extract_exif(&self, path: &Path) -> Result<Option<ExifData>, CoreError> {
        todo!()
    }
}

#[async_trait]
impl MetadataExtractor for ImageExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Image]
    }

    fn supported_mime_types(&self) -> Option<&[&str]> {
        Some(&["image/jpeg", "image/png", "image/gif", "image/webp", "image/tiff"])
    }

    async fn extract(&self, path: &Path) -> Result<ExtendedMetadata, CoreError> {
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
