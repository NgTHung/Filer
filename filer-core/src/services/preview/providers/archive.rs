use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeInfo};
use crate::services::preview::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Archive contents preview provider
pub struct ArchiveProvider;

impl ArchiveProvider {
    pub fn new() -> Self {
        Self
    }

    /// List ZIP contents
    async fn list_zip(&self, path: &Path, max_entries: usize) -> Result<PreviewData, CoreError> {
        todo!()
    }

    /// List TAR contents
    async fn list_tar(&self, path: &Path, max_entries: usize) -> Result<PreviewData, CoreError> {
        todo!()
    }
}

#[async_trait]
impl PreviewProvider for ArchiveProvider {
    fn supported_categories(&self) -> &[MimeCategory] {
        &[MimeCategory::Archive]
    }

    async fn generate(
        &self,
        path: &Path,
        mime: &MimeInfo,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        todo!()
    }

    fn name(&self) -> &'static str {
        "archive"
    }
}

impl Default for ArchiveProvider {
    fn default() -> Self {
        Self::new()
    }
}