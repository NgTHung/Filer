use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Audio/video metadata and preview provider
pub struct MediaProvider;

impl MediaProvider {
    pub fn new() -> Self {
        Self
    }

    /// Extract audio metadata and album art
    async fn extract_audio(&self, _path: &Path) -> Result<PreviewData, CoreError> {
        todo!()
    }

    /// Extract video thumbnail
    async fn extract_video(&self, _path: &Path, _options: &PreviewOptions) -> Result<PreviewData, CoreError> {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for MediaProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio, MimeCategory::Video]
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
        "media"
    }
}

impl Default for MediaProvider {
    fn default() -> Self {
        Self::new()
    }
}