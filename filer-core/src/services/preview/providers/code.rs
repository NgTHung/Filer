use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Syntax-highlighted code preview provider
pub struct CodeProvider {
    // Syntect highlighter would be stored here
}

impl CodeProvider {
    pub fn new() -> Self {
        Self {}
    }

    /// Detect language from file extension
    fn detect_language(&self, _path: &Path) -> Option<String> {
        todo!()
    }

    /// Highlight code with syntect
    fn highlight(&self, _code: &str, _language: &str, _theme: &str) -> String {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for CodeProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    async fn generate(
        &self,
        _path: &Path,
        _mime: &MimeInfo,
        _options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        todo!()
    }

    fn priority(&self) -> u8 {
        100 // Higher priority than plain text
    }

    fn name(&self) -> &'static str {
        "code"
    }
}

impl Default for CodeProvider {
    fn default() -> Self {
        Self::new()
    }
}