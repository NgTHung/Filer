use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::MimeCategory;

use super::extended::ExtendedMetadata;

/// Trait for metadata extractors
#[async_trait]
pub trait MetadataExtractor: Send + Sync {
    /// Categories this extractor handles
    fn supported_categories(&self) -> &[MimeCategory];

    /// Specific MIME types supported (optional, for fine-grained control)
    fn supported_mime_types(&self) -> Option<&[&str]> {
        None
    }

    /// Extract metadata from file
    async fn extract(&self, path: &Path) -> Result<ExtendedMetadata, CoreError>;

    /// Extractor name for debugging
    fn name(&self) -> &'static str;
}

/// Registry of metadata extractors
pub struct MetadataRegistry {
    extractors: Arc<scc::HashMap<MimeCategory, Vec<Box<dyn MetadataExtractor>>>>,
}

impl MetadataRegistry {
    pub fn new() -> Self {
        Self {
            extractors: Arc::new(scc::HashMap::new()),
        }
    }

    /// Create registry with all built-in extractors
    pub fn with_defaults() -> Self {
        todo!()
    }

    /// Register an extractor
    pub fn register(&mut self, extractor: Box<dyn MetadataExtractor>) {
        todo!()
    }

    /// Get extractor for a category
    pub fn get(&self, category: MimeCategory) -> Option<&dyn MetadataExtractor> {
        todo!()
    }

    /// Extract metadata using appropriate extractor
    pub async fn extract(&self, path: &Path, category: MimeCategory) -> Result<ExtendedMetadata, CoreError> {
        todo!()
    }
}

impl Default for MetadataRegistry {
    fn default() -> Self {
        Self::new()
    }
}