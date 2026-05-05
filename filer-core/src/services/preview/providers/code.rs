use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

pub struct CodeProvider;

impl CodeProvider {
    pub fn new() -> Self {
        Self
    }

    fn detect_language(path: &Path) -> Option<&'static str> {
        match path.extension()?.to_str()? {
            "rs" => Some("Rust"),
            "py" => Some("Python"),
            "js" => Some("JavaScript"),
            "ts" => Some("TypeScript"),
            "go" => Some("Go"),
            "c" | "h" => Some("C"),
            "cpp" | "cc" | "cxx" | "hpp" => Some("C++"),
            "java" => Some("Java"),
            "rb" => Some("Ruby"),
            "sh" | "bash" => Some("Bash"),
            "toml" => Some("TOML"),
            "json" => Some("JSON"),
            "yaml" | "yml" => Some("YAML"),
            "html" | "htm" => Some("HTML"),
            "css" => Some("CSS"),
            "md" => Some("Markdown"),
            "sql" => Some("SQL"),
            "xml" => Some("XML"),
            _ => None,
        }
    }

    #[cfg(feature = "preview-code")]
    fn highlight(code: &str, language: &str, theme: &str) -> String {
        use syntect::easy::HighlightLines;
        use syntect::highlighting::ThemeSet;
        use syntect::html::{IncludeBackground, styled_line_to_highlighted_html};
        use syntect::parsing::SyntaxSet;
        use syntect::util::LinesWithEndings;

        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let syntax = ss
            .find_syntax_by_name(language)
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        let theme_obj = ts
            .themes
            .get(theme)
            .or_else(|| ts.themes.get("base16-ocean.dark"))
            .unwrap_or_else(|| ts.themes.values().next().unwrap());

        let mut highlighter = HighlightLines::new(syntax, theme_obj);
        let mut output = String::new();

        for line in LinesWithEndings::from(code) {
            if let Ok(ranges) = highlighter.highlight_line(line, &ss) {
                let html = styled_line_to_highlighted_html(&ranges, IncludeBackground::No)
                    .unwrap_or_else(|_| line.to_string());
                output.push_str(&html);
            }
        }
        output
    }
}

#[async_trait]
impl PreviewProvider for CodeProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Text]
    }

    async fn generate(
        &self,
        path: &Path,
        _mime: &MimeInfo,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;

        let mut buf = vec![0u8; options.max_bytes];
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        buf.truncate(n);

        let truncated = n == options.max_bytes;
        let content = String::from_utf8_lossy(&buf).into_owned();
        let total_lines = content.lines().count();

        #[cfg(feature = "preview-code")]
        if let Some(lang) = Self::detect_language(path) {
            let highlighted = Self::highlight(&content, lang, &options.syntax_theme);
            return Ok(PreviewData::HighlightedText {
                content: highlighted,
                language: lang.to_string(),
                theme: options.syntax_theme.clone(),
                truncated,
            });
        }

        Ok(PreviewData::Text {
            content,
            truncated,
            total_lines,
        })
    }

    fn priority(&self) -> u8 {
        100
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
