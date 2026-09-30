use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;

#[cfg(feature = "preview-code")]
use syntect::highlighting::ThemeSet;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

#[cfg(feature = "preview-code")]
pub struct CodeProvider {
    themes: ThemeSet,
}

#[cfg(not(feature = "preview-code"))]
pub struct CodeProvider;

impl CodeProvider {
    #[cfg(feature = "preview-code")]
    pub fn new() -> Self {
        Self {
            themes: ThemeSet::load_defaults(),
        }
    }

    #[cfg(not(feature = "preview-code"))]
    pub fn new() -> Self {
        Self
    }

    #[cfg(feature = "preview-code")]
    /// Creates a provider with the supplied syntax theme catalog.
    pub fn with_theme_set(themes: ThemeSet) -> Self {
        Self { themes }
    }

    #[cfg(feature = "preview-code")]
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
    fn highlight(&self, code: &str, language: &str, theme: &str) -> String {
        use syntect::easy::HighlightLines;
        use syntect::html::{IncludeBackground, styled_line_to_highlighted_html};
        use syntect::parsing::SyntaxSet;
        use syntect::util::LinesWithEndings;

        let ss = SyntaxSet::load_defaults_newlines();

        let syntax = ss
            .find_syntax_by_name(language)
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        let theme_obj = self
            .themes
            .themes
            .get(theme)
            .or_else(|| self.themes.themes.get("base16-ocean.dark"))
            .or_else(|| self.themes.themes.values().next());

        let Some(theme_obj) = theme_obj else {
            return code.to_owned();
        };

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
            let highlighted = self.highlight(&content, lang, &options.syntax_theme);
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
