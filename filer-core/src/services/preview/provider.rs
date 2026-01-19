use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::services::mime::MimeCategory;

/// Generated preview data
#[derive(Debug, Clone)]
pub enum PreviewData {
    /// Plain text content
    Text {
        content: String,
        truncated: bool,
        total_lines: usize,
    },

    /// Syntax highlighted text (HTML or terminal codes)
    HighlightedText {
        content: String,
        language: String,
        theme: String,
        truncated: bool,
    },

    /// Image thumbnail
    Image {
        data: Vec<u8>,
        format: ImageFormat,
        width: u32,
        height: u32,
        original_width: u32,
        original_height: u32,
    },

    /// Audio waveform or album art
    Audio {
        waveform: Option<Vec<f32>>,
        album_art: Option<Vec<u8>>,
        duration_secs: f64,
    },

    /// Video thumbnail(s)
    Video {
        thumbnails: Vec<VideoThumbnail>,
        duration_secs: f64,
    },

    /// Document pages preview
    Document {
        pages: Vec<DocumentPage>,
        total_pages: usize,
    },

    /// Archive contents listing
    Archive {
        entries: Vec<ArchivePreviewEntry>,
        total_entries: usize,
        truncated: bool,
    },

    /// Binary hex dump
    Binary {
        hex_dump: String,
        size: u64,
    },

    /// Preview not available
    Unsupported {
        mime_type: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
}

#[derive(Debug, Clone)]
pub struct VideoThumbnail {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub timestamp_secs: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct DocumentPage {
    pub page_number: usize,
    pub image: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ArchivePreviewEntry {
    pub path: String,
    pub size: u64,
    pub is_directory: bool,
}

/// Options for preview generation
#[derive(Debug, Clone)]
pub struct PreviewOptions {
    /// Maximum width for image thumbnails
    pub max_width: u32,
    /// Maximum height for image thumbnails
    pub max_height: u32,
    /// Maximum lines for text preview
    pub max_lines: usize,
    /// Maximum bytes to read
    pub max_bytes: usize,
    /// Image format for generated thumbnails
    pub output_format: ImageFormat,
    /// Syntax highlighting theme
    pub syntax_theme: String,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            max_width: 400,
            max_height: 400,
            max_lines: 100,
            max_bytes: 1024 * 1024, // 1MB
            output_format: ImageFormat::Png,
            syntax_theme: "base16-ocean.dark".to_string(),
        }
    }
}

/// Trait for preview providers
#[async_trait]
pub trait PreviewProvider: Send + Sync {
    /// Categories this provider handles
    fn supported_categories(&self) -> &[MimeCategory];

    /// File extensions supported (optional, for fine-grained control)
    fn supported_extensions(&self) -> Option<&[&str]> {
        None
    }

    /// Generate preview for a file
    async fn generate(
        &self,
        path: &Path,
        options: &PreviewOptions,
    ) -> Result<PreviewData, CoreError>;

    /// Priority (higher = preferred when multiple providers match)
    fn priority(&self) -> u8 {
        100
    }

    /// Provider name for debugging
    fn name(&self) -> &'static str;
}