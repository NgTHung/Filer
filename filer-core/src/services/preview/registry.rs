use std::collections::HashMap;
use std::path::Path;

use crate::errors::CoreError;
use crate::services::mime::{MimeCategory, MimeDetector};

use super::provider::{PreviewData, PreviewOptions, PreviewProvider};

/// Registry of preview providers
pub struct PreviewRegistry {
    providers: HashMap<MimeCategory, Vec<Box<dyn PreviewProvider>>>,
    mime_detector: MimeDetector,
    default_options: PreviewOptions,
}

impl PreviewRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            mime_detector: MimeDetector::new(),
            default_options: PreviewOptions::default(),
        }
    }

    /// Create registry with all built-in providers
    pub fn with_defaults() -> Self {
        todo!()
    }

    /// Register a preview provider
    pub fn register(&mut self, provider: Box<dyn PreviewProvider>) {
        todo!()
    }

    /// Set default preview options
    pub fn set_default_options(&mut self, options: PreviewOptions) {
        self.default_options = options;
    }

    /// Generate preview for a file
    pub async fn generate(&self, path: &Path) -> Result<PreviewData, CoreError> {
        self.generate_with_options(path, &self.default_options).await
    }

    /// Generate preview with custom options
    pub async fn generate_with_options(
        &self,
        path: &Path,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError> {
        todo!()
    }

    /// Check if preview is available for a path
    pub fn can_preview(&self, path: &Path) -> bool {
        todo!()
    }

    /// Get best provider for a category
    fn get_provider(&self, category: MimeCategory) -> Option<&dyn PreviewProvider> {
        todo!()
    }
}

impl Default for PreviewRegistry {
    fn default() -> Self {
        Self::new()
    }
}