use std::path::Path;

/// Detected MIME information
#[derive(Debug, Clone)]
pub struct MimeInfo {
    pub mime_type: String,
    pub category: MimeCategory,
    pub encoding: Option<String>,
}

/// Broad category for routing to preview providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MimeCategory {
    Text,
    Image,
    Audio,
    Video,
    Document,
    Archive,
    Binary,
    Unknown,
}

pub struct MimeDetector;

impl MimeDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect MIME type from file path (extension-based, fast)
    pub fn detect_from_path(&self, path: &Path) -> MimeInfo {
        todo!()
    }

    /// Detect MIME type from file contents (magic bytes, accurate)
    pub fn detect_from_bytes(&self, bytes: &[u8]) -> MimeInfo {
        todo!()
    }

    /// Detect with both path hint and content
    pub fn detect(&self, path: &Path, bytes: &[u8]) -> MimeInfo {
        todo!()
    }

    /// Get category from MIME type string
    pub fn categorize(mime_type: &str) -> MimeCategory {
        todo!()
    }
}

impl Default for MimeDetector {
    fn default() -> Self {
        Self::new()
    }
}