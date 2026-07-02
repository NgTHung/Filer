// Extended metadata extractor tests — Phase 7 specification.
//
// These tests define the required behavior of each `MetadataExtractor`
// and the `MetadataRegistry`. All extractor `extract()` tests will panic
// with `not yet implemented` until the implementations replace their
// `todo!()` calls.
//
// Test groups:
//   - `registry_tests`          — routing, registration, Unavailable fallback
//   - `image_extractor_tests`   — PNG/JPEG dimensions, EXIF presence
//   - `audio_extractor_tests`   — duration, sample rate, ID3 tags
//   - `video_extractor_tests`   — dimensions, duration, format field
//   - `document_extractor_tests`— PDF page count, title field
//   - `archive_extractor_tests` — ZIP entry listing, file count
//   - `code_extractor_tests`    — language detection, line count

use std::io::Write;

use crate::model::registry::NodeRegistry;
use crate::services::metadata::extractor::MetadataExtractor; // trait must be in scope
use crate::services::metadata::extractors::{
    ArchiveExtractor, AudioExtractor, CodeExtractor, DocumentExtractor, ImageExtractor,
    VideoExtractor,
};
use crate::services::metadata::{ExtendedMetadata, MetadataRegistry};
use crate::services::mime::{DetectionConfidence, MimeCategory, MimeInfo};
use crate::vfs::local::LocalFs;

fn mime(mime_type: &str, category: MimeCategory) -> MimeInfo {
    MimeInfo {
        mime_type: mime_type.to_string(),
        category,
        encoding: None,
        confidence: DetectionConfidence::Definitive,
    }
}
fn local_provider() -> LocalFs {
    LocalFs::new(NodeRegistry::new())
}

/// Write `bytes` to a NamedTempFile with `suffix` and return it.
/// The file is kept alive as long as the returned value is in scope.
fn temp_file_with(bytes: &[u8], suffix: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(suffix).tempfile().unwrap();
    f.write_all(bytes).unwrap();
    f
}

/// Minimal 1×1 transparent PNG (67 bytes, valid per PNG spec).
///
/// IHDR: width=1, height=1, 8-bit RGBA (color type 6).
/// IDAT: zlib-compressed single transparent pixel.
fn png_1x1() -> Vec<u8> {
    vec![
        // PNG signature
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // IHDR chunk (length=13)
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, // width  = 1
        0x00, 0x00, 0x00, 0x01, // height = 1
        0x08, 0x06, // 8-bit RGBA
        0x00, 0x00, 0x00, // compression / filter / interlace
        0x1F, 0x15, 0xC4, 0x89, // CRC
        // IDAT chunk (length=10)
        0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, // CRC
        // IEND chunk (length=0)
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // CRC
    ]
}

/// Minimal JPEG: SOI + JFIF APP0 marker + EOI.
fn jpeg_minimal() -> Vec<u8> {
    vec![
        0xFF, 0xD8, // SOI
        0xFF, 0xE0, 0x00, 0x10, // APP0 marker + length
        0x4A, 0x46, 0x49, 0x46, 0x00, // "JFIF\0"
        0x01, 0x01, // version 1.1
        0x00, // aspect-ratio units
        0x00, 0x01, 0x00, 0x01, // X / Y density
        0x00, 0x00, // thumbnail dimensions
        0xFF, 0xD9, // EOI
    ]
}

/// Minimal ID3v2.3 tag with zero frames (MP3 file stub).
fn mp3_id3_header() -> Vec<u8> {
    vec![
        0x49, 0x44, 0x33, // "ID3"
        0x03, 0x00, // version 2.3, revision 0
        0x00, // flags
        0x00, 0x00, 0x00, 0x00, // syncsafe tag size = 0 (no frames)
    ]
}

/// OGG stream capture pattern (first 14 bytes of a valid OGG page).
fn ogg_capture() -> Vec<u8> {
    b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00".to_vec()
}

/// Minimal MP4 ftyp box (28 bytes) — isom/iso2/avc1 compatible brands.
fn mp4_ftyp() -> Vec<u8> {
    vec![
        0x00, 0x00, 0x00, 0x1C, // box size = 28
        0x66, 0x74, 0x79, 0x70, // "ftyp"
        0x69, 0x73, 0x6F, 0x6D, // major brand  "isom"
        0x00, 0x00, 0x02, 0x00, // minor version
        0x69, 0x73, 0x6F, 0x6D, // compatible   "isom"
        0x69, 0x73, 0x6F, 0x32, // compatible   "iso2"
        0x61, 0x76, 0x63, 0x31, // compatible   "avc1"
    ]
}

/// Minimal valid PDF with one empty page (no /Title).
fn pdf_one_page() -> Vec<u8> {
    b"%PDF-1.4\n\
      1 0 obj<</Type /Catalog /Pages 2 0 R>>endobj\n\
      2 0 obj<</Type /Pages /Kids [3 0 R] /Count 1>>endobj\n\
      3 0 obj<</Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]>>endobj\n\
      xref\n\
      0 4\n\
      0000000000 65535 f \n\
      0000000009 00000 n \n\
      0000000054 00000 n \n\
      0000000107 00000 n \n\
      trailer<</Size 4 /Root 1 0 R>>\n\
      startxref\n\
      174\n\
      %%EOF\n"
        .to_vec()
}

/// Minimal empty ZIP (End of Central Directory record only, 0 entries).
fn zip_empty() -> Vec<u8> {
    vec![
        0x50, 0x4B, 0x05, 0x06, // EOCD signature
        0x00, 0x00, // disk number
        0x00, 0x00, // disk with CD
        0x00, 0x00, // entries on disk
        0x00, 0x00, // total entries
        0x00, 0x00, 0x00, 0x00, // CD size
        0x00, 0x00, 0x00, 0x00, // CD offset
        0x00, 0x00, // comment length
    ]
}

