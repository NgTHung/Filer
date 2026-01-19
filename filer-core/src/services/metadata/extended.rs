use std::collections::HashMap;

/// Extended metadata (type-specific)
#[derive(Debug, Clone)]
pub enum ExtendedMetadata {
    Image(ImageMetadata),
    Audio(AudioMetadata),
    Video(VideoMetadata),
    Document(DocumentMetadata),
    Archive(ArchiveMetadata),
    Code(CodeMetadata),
    None,
}

#[derive(Debug, Clone)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: Option<String>,
    pub bit_depth: Option<u8>,
    pub has_alpha: bool,
    pub exif: Option<ExifData>,
}

#[derive(Debug, Clone)]
pub struct ExifData {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub date_taken: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub exposure_time: Option<String>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub focal_length: Option<f64>,
    pub orientation: Option<u32>,
    pub raw: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub duration_secs: f64,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_rate: Option<u32>,
    pub format: String,
    pub tags: AudioTags,
}

#[derive(Debug, Clone, Default)]
pub struct AudioTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
    pub genre: Option<String>,
    pub album_art: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub width: u32,
    pub height: u32,
    pub duration_secs: f64,
    pub frame_rate: Option<f32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub format: String,
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub page_count: Option<u32>,
    pub word_count: Option<u32>,
    pub created: Option<String>,
    pub modified: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ArchiveMetadata {
    pub format: String,
    pub file_count: usize,
    pub total_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f32,
    pub entries: Vec<ArchiveEntry>,
}

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub compressed_size: u64,
    pub is_directory: bool,
}

#[derive(Debug, Clone)]
pub struct CodeMetadata {
    pub language: String,
    pub line_count: usize,
    pub has_syntax_errors: bool,
}