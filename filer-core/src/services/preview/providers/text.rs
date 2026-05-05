use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;

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
