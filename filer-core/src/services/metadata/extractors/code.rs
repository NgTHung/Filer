use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::metadata::CodeMetadata;
use crate::services::metadata::extended::ExtendedMetadata;
use crate::services::metadata::extractor::MetadataExtractor;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::vfs::provider::FsProvider;

/// Code/text metadata extractor (language, line count)
pub struct CodeExtractor;

impl CodeExtractor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MetadataExtractor for CodeExtractor {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    async fn extract(
        &self,
        path: &Path,
        mime: &MimeInfo,
        provider: &dyn FsProvider,
    ) -> Result<ExtendedMetadata, CoreError> {
        if mime.category != MimeCategory::Text {
            return Err(CoreError::invalid_data(
                "Invalid type of extractor".to_string(),
            ));
        }
        let lang = match mime.mime_type.as_str() {
            "text/x-c" => "C",
            "text/x-asm" => "Assembly",
            "text/x-csharp" => "C#",
            "text/x-java" => "Java",
            "text/javascript" => "JS",
            "text/typescript" => "TS",
            "text/x-kotlin" => "Kotlin",
            "text/x-lua" => "Lua",
            "text/x-objcsrc" => "Objcsrc",
            "text/x-nim" => "Nim",
            "text/x-python" => "Python",
            "text/x-ruby" => "Ruby",
            "text/x-rust" => "Rust",
            "text/x-sql" => "SQL",
            "text/x-swift" => "Swift",
            "text/x-tex" => "Latex",
            "text/css" => "CSS",
            "text/html" => "HTML",
            _ => "Text",
        };

        let s = provider.read(path).await.unwrap_or_default();
        let lines = s.iter().filter(|v| **v == b'\n').count();

        Ok(ExtendedMetadata::Code(CodeMetadata {
            language: lang.to_string(),
            line_count: lines,
            has_syntax_errors: false,
        }))
    }

    fn name(&self) -> &'static str {
        "code"
    }
}

impl Default for CodeExtractor {
    fn default() -> Self {
        Self::new()
    }
}
