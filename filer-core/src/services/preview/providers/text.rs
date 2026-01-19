use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::MimeCategory;
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Plain text preview provider
pub struct TextProvider;

impl TextProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PreviewProvider for TextProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    fn supported_extensions(&self) -> Option<&[&str]> {
        Some(&["txt", "log", "csv", "tsv", "ini", "conf", "cfg"])
    }

    async fn generate(
        &self,
        path: &Path,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        todo!()
    }

    fn priority(&self) -> u8 {
        50 // Lower priority, CodeProvider handles code files
    }

    fn name(&self) -> &'static str {
        "text"
    }
}

impl Default for TextProvider {
    fn default() -> Self {
        Self::new()
    }
}