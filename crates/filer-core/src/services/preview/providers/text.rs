use std::path::Path;

use crate::actors::native::{file_io, read_once};
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

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

    async fn generate(
        &self,
        path: &Path,
        _mime: &MimeInfo,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        let limit = options.max_bytes;
        let buf = file_io(path, move |path| {
            read_once(&mut std::fs::File::open(path)?, limit)
        })
        .await?;
        let truncated = buf.len() == limit;
        let content = String::from_utf8_lossy(&buf).into_owned();
        let total_lines = content.lines().count();

        Ok(PreviewData::Text {
            content,
            truncated,
            total_lines,
        })
    }

    fn priority(&self) -> u8 {
        50
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
