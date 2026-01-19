use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::MimeCategory;
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Image thumbnail preview provider
pub struct ImageProvider;

impl ImageProvider {
    pub fn new() -> Self {
        Self
    }

    /// Generate thumbnail maintaining aspect ratio
    fn generate_thumbnail(
        &self,
        path: &Path,
        max_width: u32,
        max_height: u32,
    ) -> Result<(Vec<u8>, u32, u32, u32, u32), CoreError> {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for ImageProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Image]
    }

    fn supported_extensions(&self) -> Option<&[&str]> {
        Some(&["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "tif"])
    }

    async fn generate(
        &self,
        path: &Path,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        todo!()
    }

    fn name(&self) -> &'static str {
        "image"
    }
}

impl Default for ImageProvider {
    fn default() -> Self {
        Self::new()
    }
}