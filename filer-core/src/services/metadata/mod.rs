mod basic;
mod extended;
mod extractor;
pub mod extractors;

pub use basic::{BasicMetadata, Permissions};
pub use extended::{
    ArchiveEntry, ArchiveMetadata, AudioMetadata, AudioTags, CodeMetadata, DocumentMetadata,
    ExifData, ExtendedMetadata, ImageMetadata, VideoMetadata,
};
pub use extractor::{MetadataExtractor, MetadataRegistry};