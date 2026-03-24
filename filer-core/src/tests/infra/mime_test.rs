//! MIME detector tests — Phase 7 specification.
//!
//! These tests define the required behavior of `MimeDetector`.
//! All tests except those in `ambiguous_extension_tests` will panic with
//! `not yet implemented` until the implementation is complete.
//!
//! Test groups:
//!   - `detect_from_path_tests`    — extension-based detection
//!   - `detect_from_bytes_tests`   — magic-byte detection
//!   - `detect_tests`              — combined (ext + magic, magic wins on disagreement)
//!   - `detect_with_strategy_tests`— strategy dispatch
//!   - `categorize_tests`          — MIME string → MimeCategory
//!   - `ambiguous_extension_tests` — is_ambiguous_extension (runs today, no todo)

use std::path::Path;

use crate::services::mime::{
    DetectionConfidence, DetectionStrategy, MimeCategory, MimeDetector,
};

// ── Helpers ───────────────────────────────────────────────────────────────────


fn png_header() -> Vec<u8> {
    vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
}

fn jpeg_header() -> Vec<u8> {
    vec![0xFF, 0xD8, 0xFF, 0xE0]
}

fn gif_header() -> Vec<u8> {
    b"GIF89a".to_vec()
}

fn pdf_header() -> Vec<u8> {
    b"%PDF-1.4".to_vec()
}

fn zip_header() -> Vec<u8> {
    vec![0x50, 0x4B, 0x03, 0x04]
}

fn gzip_header() -> Vec<u8> {
    vec![0x1F, 0x8B, 0x08]
}

fn mp3_header() -> Vec<u8> {
    b"ID3\x03\x00".to_vec()
}

fn ogg_header() -> Vec<u8> {
    b"OggS\x00\x02".to_vec()
}

fn elf_header() -> Vec<u8> {
    let mut res = vec![0x7F, 0x45, 0x4C, 0x46];
    res.append(&mut vec![0x32;100]);
    res
    // vec![0x7F, 0x45, 0x4C, 0x46] // \x7FELF
}

// ── detect_from_path ─────────────────────────────────────────────────────────

#[cfg(test)]
mod detect_from_path_tests {
    use super::*;

    #[test]
    fn png_extension_is_image() {
        let info = MimeDetector::detect_from_path(Path::new("photo.png"));
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/png");
        assert_ne!(info.confidence, DetectionConfidence::Unknown);
    }

    #[test]
    fn jpg_extension_is_image() {
        let info = MimeDetector::detect_from_path(Path::new("photo.jpg"));
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/jpeg");
    }

    #[test]
    fn mp3_extension_is_audio() {
        let info = MimeDetector::detect_from_path(Path::new("song.mp3"));
        assert_eq!(info.category, MimeCategory::Audio);
        assert_eq!(info.mime_type, "audio/mpeg");
    }

    #[test]
    fn mp4_extension_is_video() {
        let info = MimeDetector::detect_from_path(Path::new("clip.mp4"));
        assert_eq!(info.category, MimeCategory::Video);
        assert_eq!(info.mime_type, "video/mp4");
    }

    #[test]
    fn zip_extension_is_archive() {
        let info = MimeDetector::detect_from_path(Path::new("archive.zip"));
        assert_eq!(info.category, MimeCategory::Archive);
        assert_eq!(info.mime_type, "application/zip");
    }

    #[test]
    fn pdf_extension_is_document() {
        let info = MimeDetector::detect_from_path(Path::new("doc.pdf"));
        assert_eq!(info.category, MimeCategory::Document);
        assert_eq!(info.mime_type, "application/pdf");
    }

    #[test]
    fn rs_extension_is_text() {
        let info = MimeDetector::detect_from_path(Path::new("main.rs"));
        assert_eq!(info.category, MimeCategory::Text);
    }

    #[test]
    fn no_extension_is_unknown_confidence() {
        let info = MimeDetector::detect_from_path(Path::new("Makefile"));
        assert_eq!(info.confidence, DetectionConfidence::Unknown);
    }

    #[test]
    fn bin_extension_is_unknown_confidence() {
        let info = MimeDetector::detect_from_path(Path::new("data.bin"));
        assert_eq!(info.confidence, DetectionConfidence::Unknown);
    }

    #[test]
    fn txt_extension_is_probable_not_definitive() {
        // .txt is ambiguous (could be CSV, JSON, TOML) → never Definitive
        let info = MimeDetector::detect_from_path(Path::new("notes.txt"));
        assert_ne!(info.confidence, DetectionConfidence::Definitive);
        assert_eq!(info.category, MimeCategory::Text);
    }

    #[test]
    fn gif_extension_is_image() {
        let info = MimeDetector::detect_from_path(Path::new("anim.gif"));
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/gif");
    }

    #[test]
    fn mkv_extension_is_video() {
        let info = MimeDetector::detect_from_path(Path::new("movie.mkv"));
        assert_eq!(info.category, MimeCategory::Video);
    }
}

// ── detect_from_bytes ─────────────────────────────────────────────────────────

#[cfg(test)]
mod detect_from_bytes_tests {
    use std::f32::consts::E;

    use super::*;

    #[test]
    fn png_magic_is_image_definitive() {
        let info = MimeDetector::detect_from_bytes(&png_header());
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/png");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn jpeg_magic_is_image_definitive() {
        let info = MimeDetector::detect_from_bytes(&jpeg_header());
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/jpeg");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn gif_magic_is_image_definitive() {
        let info = MimeDetector::detect_from_bytes(&gif_header());
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/gif");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn pdf_magic_is_document_definitive() {
        let info = MimeDetector::detect_from_bytes(&pdf_header());
        assert_eq!(info.category, MimeCategory::Document);
        assert_eq!(info.mime_type, "application/pdf");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn zip_magic_is_archive_definitive() {
        let info = MimeDetector::detect_from_bytes(&zip_header());
        assert_eq!(info.category, MimeCategory::Archive);
        assert_eq!(info.mime_type, "application/zip");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn gzip_magic_is_archive_definitive() {
        let info = MimeDetector::detect_from_bytes(&gzip_header());
        assert_eq!(info.category, MimeCategory::Archive);
        assert_eq!(info.mime_type, "application/gzip");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn mp3_id3_magic_is_audio_definitive() {
        let info = MimeDetector::detect_from_bytes(&mp3_header());
        assert_eq!(info.category, MimeCategory::Audio);
        assert_eq!(info.mime_type, "audio/mpeg");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn ogg_magic_is_audio_definitive() {
        let info = MimeDetector::detect_from_bytes(&ogg_header());
        assert_eq!(info.category, MimeCategory::Audio);
        assert_eq!(info.mime_type, "audio/ogg");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn elf_magic_is_binary_definitive() {
        let info = MimeDetector::detect_from_bytes(&elf_header());
        println!("{}",infer::app::is_elf(&elf_header()));
        assert_eq!(info.category, MimeCategory::Binary);
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn empty_bytes_are_unknown() {
        let info = MimeDetector::detect_from_bytes(&[]);
        assert_eq!(info.confidence, DetectionConfidence::Unknown);
    }
}

// ── detect (combined) ────────────────────────────────────────────────────────

#[cfg(test)]
mod detect_tests {
    use super::*;

    #[test]
    fn matching_ext_and_magic_returns_definitive() {
        // .png extension + PNG magic → agree → Definitive
        let info = MimeDetector::detect(Path::new("photo.png"), &png_header());
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/png");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn magic_wins_over_wrong_extension() {
        // .txt extension says Text, but PNG magic says Image → magic wins
        let info = MimeDetector::detect(Path::new("file.txt"), &png_header());
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/png");
        assert_eq!(info.confidence, DetectionConfidence::Definitive);
    }

    #[test]
    fn inconclusive_magic_falls_back_to_path() {
        // .dat (ambiguous ext) + empty bytes (inconclusive magic) → Unknown confidence
        let info = MimeDetector::detect(Path::new("data.dat"), &[]);
        assert_eq!(info.confidence, DetectionConfidence::Unknown);
    }
}

// ── detect_with_strategy ──────────────────────────────────────────────────────

#[cfg(test)]
mod detect_with_strategy_tests {
    use super::*;

    #[test]
    fn extension_only_never_uses_header() {
        // Even with a PNG header, ExtensionOnly must return the extension result
        let info = MimeDetector::detect_with_strategy(
            Path::new("notes.txt"),
            Some(&png_header()),
            DetectionStrategy::ExtensionOnly,
        );
        // Must be Text (from extension), not Image (from header)
        assert_eq!(info.category, MimeCategory::Text);
    }

    #[test]
    fn extension_with_fallback_trusts_known_extension() {
        // .rs is not ambiguous → extension result used, header ignored
        let info = MimeDetector::detect_with_strategy(
            Path::new("main.rs"),
            Some(&png_header()),
            DetectionStrategy::ExtensionWithFallback,
        );
        assert_eq!(info.category, MimeCategory::Text);
    }

    #[test]
    fn extension_with_fallback_uses_magic_for_ambiguous_extension() {
        // .bin is ambiguous → PNG header wins
        let info = MimeDetector::detect_with_strategy(
            Path::new("image.bin"),
            Some(&png_header()),
            DetectionStrategy::ExtensionWithFallback,
        );
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/png");
    }

    #[test]
    fn extension_with_fallback_falls_back_when_no_header() {
        // header: None (remote provider) → falls back to extension regardless of strategy
        let info = MimeDetector::detect_with_strategy(
            Path::new("image.bin"),
            None,
            DetectionStrategy::ExtensionWithFallback,
        );
        // No magic available — result comes from extension (ambiguous → Unknown confidence)
        assert_eq!(info.confidence, DetectionConfidence::Unknown);
    }

    #[test]
    fn magic_bytes_always_prefers_magic_when_header_available() {
        // .txt says Text, PNG header says Image → MagicBytes always uses magic
        let info = MimeDetector::detect_with_strategy(
            Path::new("notes.txt"),
            Some(&png_header()),
            DetectionStrategy::MagicBytes,
        );
        assert_eq!(info.category, MimeCategory::Image);
        assert_eq!(info.mime_type, "image/png");
    }
}

// ── categorize ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod categorize_tests {
    use super::*;

    #[test]
    fn image_png() {
        assert_eq!(MimeDetector::categorize("image/png"), MimeCategory::Image);
    }

    #[test]
    fn image_jpeg() {
        assert_eq!(MimeDetector::categorize("image/jpeg"), MimeCategory::Image);
    }

    #[test]
    fn audio_mpeg() {
        assert_eq!(MimeDetector::categorize("audio/mpeg"), MimeCategory::Audio);
    }

    #[test]
    fn video_mp4() {
        assert_eq!(MimeDetector::categorize("video/mp4"), MimeCategory::Video);
    }

    #[test]
    fn text_plain() {
        assert_eq!(MimeDetector::categorize("text/plain"), MimeCategory::Text);
    }

    #[test]
    fn text_html() {
        assert_eq!(MimeDetector::categorize("text/html"), MimeCategory::Text);
    }

    #[test]
    fn application_pdf() {
        assert_eq!(
            MimeDetector::categorize("application/pdf"),
            MimeCategory::Document
        );
    }

    #[test]
    fn application_zip() {
        assert_eq!(
            MimeDetector::categorize("application/zip"),
            MimeCategory::Archive
        );
    }

    #[test]
    fn application_octet_stream() {
        assert_eq!(
            MimeDetector::categorize("application/octet-stream"),
            MimeCategory::Binary
        );
    }

    #[test]
    fn unknown_mime_type() {
        assert_eq!(
            MimeDetector::categorize("application/x-custom-unknown"),
            MimeCategory::Unknown
        );
    }
}

// ── is_ambiguous_extension ────────────────────────────────────────────────────
// These tests run immediately — is_ambiguous_extension has a real implementation.

#[cfg(test)]
mod ambiguous_extension_tests {
    use super::*;

    #[test]
    fn empty_string_is_ambiguous() {
        assert!(MimeDetector::is_ambiguous_extension(""));
    }

    #[test]
    fn bin_is_ambiguous() {
        assert!(MimeDetector::is_ambiguous_extension("bin"));
    }

    #[test]
    fn txt_is_ambiguous() {
        assert!(MimeDetector::is_ambiguous_extension("txt"));
    }

    #[test]
    fn log_is_ambiguous() {
        assert!(MimeDetector::is_ambiguous_extension("log"));
    }

    #[test]
    fn png_is_not_ambiguous() {
        assert!(!MimeDetector::is_ambiguous_extension("png"));
    }

    #[test]
    fn rs_is_not_ambiguous() {
        assert!(!MimeDetector::is_ambiguous_extension("rs"));
    }

    #[test]
    fn mp4_is_not_ambiguous() {
        assert!(!MimeDetector::is_ambiguous_extension("mp4"));
    }
}
