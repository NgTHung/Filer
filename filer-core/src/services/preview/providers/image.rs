use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
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
        _path: &Path,
        _max_width: u32,
        _max_height: u32,
    ) -> Result<(Vec<u8>, u32, u32, u32, u32), CoreError> {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for ImageProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Image]
    }

    async fn generate(
        &self,
        _path: &Path,
        _mime: &MimeInfo,
        _options: &PreviewOptions,
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