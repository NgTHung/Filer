use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::MimeCategory;
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Audio/video metadata and preview provider
pub struct MediaProvider;

impl MediaProvider {
    pub fn new() -> Self {
        Self
    }

    /// Extract audio metadata and album art
    async fn extract_audio(&self, path: &Path) -> Result<PreviewData, CoreError> {
        todo!()
    }

    /// Extract video thumbnail
    async fn extract_video(&self, path: &Path, options: &PreviewOptions) -> Result<PreviewData, CoreError> {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for MediaProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Audio, MimeCategory::Video]
    }

    fn supported_extensions(&self) -> Option<&[&str]> {
        Some(&[
            // Audio
            "mp3", "flac", "ogg", "wav", "aac", "m4a", "wma", "opus",
            // Video
            "mp4", "mkv", "avi", "webm", "mov", "wmv", "flv",
        ])
    }

    async fn generate(
        &self,
        path: &Path,
        options: &PreviewOptions,
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