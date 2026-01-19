use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::MimeCategory;
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
    fn detect_language(&self, path: &Path) -> Option<String> {
        todo!()
    }

    /// Highlight code with syntect
    fn highlight(&self, code: &str, language: &str, theme: &str) -> String {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for CodeProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    fn supported_extensions(&self) -> Option<&[&str]> {
        Some(&[
            "rs", "py", "js", "ts", "jsx", "tsx", "go", "c", "cpp", "h", "hpp",
            "java", "kt", "swift", "rb", "php", "cs", "fs", "hs", "ml", "ex",
            "exs", "clj", "scala", "lua", "sh", "bash", "zsh", "fish", "ps1",
            "sql", "html", "css", "scss", "sass", "less", "json", "yaml", "yml",
            "toml", "xml", "md", "markdown", "dockerfile", "makefile",
        ])
    }

    async fn generate(
        &self,
        path: &Path,
        options: &PreviewOptions,
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