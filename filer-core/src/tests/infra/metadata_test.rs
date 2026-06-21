//! Extended metadata extractor tests — Phase 7 specification.
//!
//! These tests define the required behavior of each `MetadataExtractor`
//! and the `MetadataRegistry`. All extractor `extract()` tests will panic
//! with `not yet implemented` until the implementations replace their
//! `todo!()` calls.
//!
//! Test groups:
//!   - `registry_tests`          — routing, registration, Unavailable fallback
//!   - `image_extractor_tests`   — PNG/JPEG dimensions, EXIF presence
//!   - `audio_extractor_tests`   — duration, sample rate, ID3 tags
//!   - `video_extractor_tests`   — dimensions, duration, format field
//!   - `document_extractor_tests`— PDF page count, title field
//!   - `archive_extractor_tests` — ZIP entry listing, file count
//!   - `code_extractor_tests`    — language detection, line count

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

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn with_defaults_covers_all_media_categories() {
        let reg = MetadataRegistry::with_defaults();
        for cat in [
            MimeCategory::Image,
            MimeCategory::Audio,
            MimeCategory::Video,
            MimeCategory::Document,
            MimeCategory::Archive,
            MimeCategory::Text,
        ] {
            let info = mime("application/octet-stream", cat);
            assert!(
                reg.get(&info).is_some(),
                "no extractor registered for {:?}",
                cat
            );
        }
    }

    #[test]
    fn unknown_category_returns_no_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("application/x-custom", MimeCategory::Unknown);
        assert!(reg.get(&info).is_none());
    }

    #[test]
    fn binary_category_returns_no_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("application/x-executable", MimeCategory::Binary);
        assert!(reg.get(&info).is_none());
    }

    #[test]
    fn routes_image_mime_to_image_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("image/png", MimeCategory::Image);
        assert_eq!(reg.get(&info).unwrap().name(), "image");
    }

    #[test]
    fn routes_audio_mime_to_audio_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("audio/mpeg", MimeCategory::Audio);
        assert_eq!(reg.get(&info).unwrap().name(), "audio");
    }

    #[test]
    fn routes_video_mime_to_video_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("video/mp4", MimeCategory::Video);
        assert_eq!(reg.get(&info).unwrap().name(), "video");
    }

    #[test]
    fn routes_document_mime_to_document_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("application/pdf", MimeCategory::Document);
        assert_eq!(reg.get(&info).unwrap().name(), "document");
    }

    #[test]
    fn routes_archive_mime_to_archive_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("application/zip", MimeCategory::Archive);
        assert_eq!(reg.get(&info).unwrap().name(), "archive");
    }

    #[test]
    fn routes_text_mime_to_code_extractor() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("text/plain", MimeCategory::Text);
        assert_eq!(reg.get(&info).unwrap().name(), "code");
    }

    #[tokio::test]
    async fn unknown_category_extract_returns_unavailable() {
        let reg = MetadataRegistry::with_defaults();
        let info = mime("application/x-custom", MimeCategory::Unknown);
        let result = reg
            .extract(std::path::Path::new("/dev/null"), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Unavailable));
    }

    #[test]
    fn first_registered_extractor_wins_for_same_category() {
        // Register image extractor twice; only the first should be returned.
        let mut reg = MetadataRegistry::new();
        reg.register(Box::new(ImageExtractor::new()));
        reg.register(Box::new(ImageExtractor::new()));
        let info = mime("image/png", MimeCategory::Image);
        // Both have name "image", so checking name is a proxy for ordering.
        assert_eq!(reg.get(&info).unwrap().name(), "image");
    }
}

#[cfg(test)]
mod image_extractor_tests {
    use super::*;

    fn extractor() -> ImageExtractor {
        ImageExtractor::new()
    }

    #[test]
    fn supported_categories_contains_image() {
        assert!(
            extractor()
                .supported_categories()
                .contains(&MimeCategory::Image)
        );
    }

    #[test]
    fn name_is_image() {
        assert_eq!(extractor().name(), "image");
    }

    #[tokio::test]
    async fn png_returns_image_variant() {
        let f = temp_file_with(&png_1x1(), ".png");
        let info = mime("image/png", MimeCategory::Image);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Image(_)));
    }

    #[tokio::test]
    async fn png_has_correct_dimensions() {
        let f = temp_file_with(&png_1x1(), ".png");
        let info = mime("image/png", MimeCategory::Image);
        let ExtendedMetadata::Image(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Image variant");
        };
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
    }

    #[tokio::test]
    async fn png_format_string_is_png() {
        let f = temp_file_with(&png_1x1(), ".png");
        let info = mime("image/png", MimeCategory::Image);
        let ExtendedMetadata::Image(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Image variant");
        };
        assert_eq!(meta.format.to_uppercase(), "PNG");
    }

    #[tokio::test]
    async fn png_without_exif_has_none_exif() {
        let f = temp_file_with(&png_1x1(), ".png");
        let info = mime("image/png", MimeCategory::Image);
        let ExtendedMetadata::Image(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Image variant");
        };
        // Synthetic minimal PNG has no EXIF block.
        assert!(meta.exif.is_none());
    }

    #[tokio::test]
    async fn jpeg_returns_image_variant() {
        let f = temp_file_with(&jpeg_minimal(), ".jpg");
        let info = mime("image/jpeg", MimeCategory::Image);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Image(_)));
    }

    #[tokio::test]
    async fn jpeg_format_string_is_jpeg() {
        let f = temp_file_with(&jpeg_minimal(), ".jpg");
        let info = mime("image/jpeg", MimeCategory::Image);
        let ExtendedMetadata::Image(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Image variant");
        };
        let fmt = meta.format.to_uppercase();
        assert!(
            fmt == "JPEG" || fmt == "JPG",
            "unexpected format string: {}",
            meta.format
        );
    }
}

#[cfg(test)]
mod audio_extractor_tests {
    use super::*;

    fn extractor() -> AudioExtractor {
        AudioExtractor::new()
    }

    #[test]
    fn supported_categories_contains_audio() {
        assert!(
            extractor()
                .supported_categories()
                .contains(&MimeCategory::Audio)
        );
    }

    #[test]
    fn name_is_audio() {
        assert_eq!(extractor().name(), "audio");
    }

    #[tokio::test]
    async fn mp3_returns_audio_variant() {
        let f = temp_file_with(&mp3_id3_header(), ".mp3");
        let info = mime("audio/mpeg", MimeCategory::Audio);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Audio(_)));
    }

    #[tokio::test]
    async fn mp3_format_string_is_mp3() {
        let f = temp_file_with(&mp3_id3_header(), ".mp3");
        let info = mime("audio/mpeg", MimeCategory::Audio);
        let ExtendedMetadata::Audio(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Audio variant");
        };
        assert_eq!(meta.format.to_uppercase(), "MP3");
    }

    #[tokio::test]
    async fn audio_duration_is_non_negative() {
        let f = temp_file_with(&mp3_id3_header(), ".mp3");
        let info = mime("audio/mpeg", MimeCategory::Audio);
        let ExtendedMetadata::Audio(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Audio variant");
        };
        assert!(meta.duration_secs >= 0.0);
    }

    #[tokio::test]
    async fn ogg_returns_audio_variant() {
        let f = temp_file_with(&ogg_capture(), ".ogg");
        let info = mime("audio/ogg", MimeCategory::Audio);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Audio(_)));
    }

    #[tokio::test]
    async fn mp3_with_no_frames_has_empty_tags() {
        // ID3 header with zero frames → no title, artist, or album.
        let f = temp_file_with(&mp3_id3_header(), ".mp3");
        let info = mime("audio/mpeg", MimeCategory::Audio);
        let ExtendedMetadata::Audio(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Audio variant");
        };
        assert!(meta.tags.title.is_none());
        assert!(meta.tags.artist.is_none());
        assert!(meta.tags.album.is_none());
    }
}

#[cfg(test)]
mod video_extractor_tests {
    use super::*;

    fn extractor() -> VideoExtractor {
        VideoExtractor::new()
    }

    #[test]
    fn supported_categories_contains_video() {
        assert!(
            extractor()
                .supported_categories()
                .contains(&MimeCategory::Video)
        );
    }

    #[test]
    fn name_is_video() {
        assert_eq!(extractor().name(), "video");
    }

    #[tokio::test]
    async fn mp4_returns_video_variant() {
        let f = temp_file_with(&mp4_ftyp(), ".mp4");
        let info = mime("video/mp4", MimeCategory::Video);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Video(_)));
    }

    #[tokio::test]
    async fn mp4_format_string_is_mp4() {
        let f = temp_file_with(&mp4_ftyp(), ".mp4");
        let info = mime("video/mp4", MimeCategory::Video);
        let ExtendedMetadata::Video(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Video variant");
        };
        assert_eq!(meta.format.to_uppercase(), "MP4");
    }

    #[tokio::test]
    async fn video_duration_is_non_negative() {
        let f = temp_file_with(&mp4_ftyp(), ".mp4");
        let info = mime("video/mp4", MimeCategory::Video);
        let ExtendedMetadata::Video(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Video variant");
        };
        assert!(meta.duration_secs >= 0.0);
    }

    #[tokio::test]
    async fn video_exposes_width_and_height() {
        // Dimensions may be 0 for a stub file, but the fields must exist and be populated.
        let f = temp_file_with(&mp4_ftyp(), ".mp4");
        let info = mime("video/mp4", MimeCategory::Video);
        let ExtendedMetadata::Video(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Video variant");
        };
        // Just assert the fields are accessible; real values tested with real files.
        let _ = (meta.width, meta.height);
    }
}

#[cfg(test)]
mod document_extractor_tests {
    use super::*;

    fn extractor() -> DocumentExtractor {
        DocumentExtractor::new()
    }

    #[test]
    fn supported_categories_contains_document() {
        assert!(
            extractor()
                .supported_categories()
                .contains(&MimeCategory::Document)
        );
    }

    #[test]
    fn name_is_document() {
        assert_eq!(extractor().name(), "document");
    }

    #[tokio::test]
    async fn pdf_returns_document_variant() {
        let f = temp_file_with(&pdf_one_page(), ".pdf");
        let info = mime("application/pdf", MimeCategory::Document);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Document(_)));
    }

    #[tokio::test]
    async fn pdf_page_count_is_one() {
        let f = temp_file_with(&pdf_one_page(), ".pdf");
        let info = mime("application/pdf", MimeCategory::Document);
        let ExtendedMetadata::Document(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Document variant");
        };
        assert_eq!(meta.page_count, Some(1));
    }

    #[tokio::test]
    async fn pdf_title_is_none_when_absent() {
        let f = temp_file_with(&pdf_one_page(), ".pdf");
        let info = mime("application/pdf", MimeCategory::Document);
        let ExtendedMetadata::Document(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Document variant");
        };
        // Minimal PDF has no /Title key in the Info dictionary.
        assert!(meta.title.is_none());
    }
}

#[cfg(test)]
mod archive_extractor_tests {
    use super::*;

    fn extractor() -> ArchiveExtractor {
        ArchiveExtractor::new()
    }

    #[test]
    fn supported_categories_contains_archive() {
        assert!(
            extractor()
                .supported_categories()
                .contains(&MimeCategory::Archive)
        );
    }

    #[test]
    fn name_is_archive() {
        assert_eq!(extractor().name(), "archive");
    }

    #[tokio::test]
    async fn empty_zip_returns_archive_variant() {
        let f = temp_file_with(&zip_empty(), ".zip");
        let info = mime("application/zip", MimeCategory::Archive);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Archive(_)));
    }

    #[tokio::test]
    async fn empty_zip_has_zero_file_count() {
        let f = temp_file_with(&zip_empty(), ".zip");
        let info = mime("application/zip", MimeCategory::Archive);
        let ExtendedMetadata::Archive(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Archive variant");
        };
        assert_eq!(meta.file_count, 0);
        assert!(meta.entries.is_empty());
    }

    #[tokio::test]
    async fn empty_zip_format_string_is_zip() {
        let f = temp_file_with(&zip_empty(), ".zip");
        let info = mime("application/zip", MimeCategory::Archive);
        let ExtendedMetadata::Archive(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Archive variant");
        };
        assert_eq!(meta.format.to_uppercase(), "ZIP");
    }

    #[tokio::test]
    async fn empty_zip_has_zero_total_size() {
        let f = temp_file_with(&zip_empty(), ".zip");
        let info = mime("application/zip", MimeCategory::Archive);
        let ExtendedMetadata::Archive(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Archive variant");
        };
        assert_eq!(meta.total_size, 0);
    }
}

#[cfg(test)]
mod code_extractor_tests {
    use super::*;

    fn extractor() -> CodeExtractor {
        CodeExtractor::new()
    }

    #[test]
    fn supported_categories_contains_text() {
        assert!(
            extractor()
                .supported_categories()
                .contains(&MimeCategory::Text)
        );
    }

    #[test]
    fn name_is_code() {
        assert_eq!(extractor().name(), "code");
    }

    #[tokio::test]
    async fn rust_source_returns_code_variant() {
        let src = b"fn main() {\n    println!(\"hello\");\n}\n";
        let f = temp_file_with(src, ".rs");
        let info = mime("text/x-rust", MimeCategory::Text);
        let result = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap();
        assert!(matches!(result, ExtendedMetadata::Code(_)));
    }

    #[tokio::test]
    async fn rust_source_language_is_rust() {
        let src = b"fn main() {}\n";
        let f = temp_file_with(src, ".rs");
        let info = mime("text/x-rust", MimeCategory::Text);
        let ExtendedMetadata::Code(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Code variant");
        };
        assert_eq!(meta.language.to_lowercase(), "rust");
    }

    #[tokio::test]
    async fn python_source_language_is_python() {
        let src = b"def hello():\n    print('hi')\n";
        let f = temp_file_with(src, ".py");
        let info = mime("text/x-python", MimeCategory::Text);
        let ExtendedMetadata::Code(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Code variant");
        };
        assert_eq!(meta.language.to_lowercase(), "python");
    }

    #[tokio::test]
    async fn plain_text_language_is_text_or_plain() {
        let src = b"hello world\n";
        let f = temp_file_with(src, ".txt");
        let info = mime("text/plain", MimeCategory::Text);
        let ExtendedMetadata::Code(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Code variant");
        };
        let lang = meta.language.to_lowercase();
        assert!(
            lang == "text" || lang == "plain" || lang == "plaintext",
            "unexpected language: {}",
            meta.language
        );
    }

    #[tokio::test]
    async fn three_line_file_has_line_count_three() {
        let src = b"line1\nline2\nline3\n";
        let f = temp_file_with(src, ".txt");
        let info = mime("text/plain", MimeCategory::Text);
        let ExtendedMetadata::Code(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Code variant");
        };
        assert_eq!(meta.line_count, 3);
    }

    #[tokio::test]
    async fn empty_file_has_zero_lines() {
        let f = temp_file_with(b"", ".txt");
        let info = mime("text/plain", MimeCategory::Text);
        let ExtendedMetadata::Code(meta) = extractor()
            .extract(f.path(), &info, &local_provider(), &crate::ProviderCx::none())
            .await
            .unwrap()
        else {
            panic!("expected Code variant");
        };
        assert_eq!(meta.line_count, 0);
    }
}
