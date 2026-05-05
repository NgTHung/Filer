//! Extension lookup table tests.
//!
//! Verifies that `EXT_TABLE` is correctly sorted (required for binary search)
//! and that key entries return the expected MIME type and category.

use crate::services::mime::table::lookup_extension;
use crate::services::mime::{MimeCategory, MimeDetector};

// ── Structural integrity ───────────────────────────────────────────────────────

#[test]
fn table_is_sorted() {
    // Pull all extension keys and compare to a freshly-sorted copy.
    // If this fails, binary_search_by_key will silently miss entries.
    use crate::services::mime::table::EXT_TABLE;
    let keys: Vec<&str> = EXT_TABLE.iter().map(|(k, _)| *k).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "EXT_TABLE must be sorted for binary search");
}

// ── Code-file extensions (new_mime_guess gets these wrong) ────────────────────

#[test]
fn py_returns_x_python() {
    let entry = lookup_extension("py").expect("py must be in table");
    assert_eq!(entry.mime_type, "text/x-python");
    assert_eq!(entry.category, MimeCategory::Text);
}

#[test]
fn rs_returns_x_rust() {
    let entry = lookup_extension("rs").expect("rs must be in table");
    assert_eq!(entry.mime_type, "text/x-rust");
    assert_eq!(entry.category, MimeCategory::Text);
}

#[test]
fn go_returns_x_go() {
    let entry = lookup_extension("go").expect("go must be in table");
    assert_eq!(entry.mime_type, "text/x-go");
}

#[test]
fn ts_returns_typescript() {
    let entry = lookup_extension("ts").expect("ts must be in table");
    assert_eq!(entry.mime_type, "text/typescript");
}

// ── Correctness spot-checks ────────────────────────────────────────────────────

#[test]
fn png_returns_image_png() {
    let entry = lookup_extension("png").expect("png must be in table");
    assert_eq!(entry.mime_type, "image/png");
    assert_eq!(entry.category, MimeCategory::Image);
}

#[test]
fn jpg_returns_image_jpeg() {
    let entry = lookup_extension("jpg").expect("jpg must be in table");
    assert_eq!(entry.mime_type, "image/jpeg");
}

#[test]
fn mp3_returns_audio_mpeg() {
    let entry = lookup_extension("mp3").expect("mp3 must be in table");
    assert_eq!(entry.mime_type, "audio/mpeg");
    assert_eq!(entry.category, MimeCategory::Audio);
}

#[test]
fn mp4_returns_video_mp4() {
    let entry = lookup_extension("mp4").expect("mp4 must be in table");
    assert_eq!(entry.mime_type, "video/mp4");
    assert_eq!(entry.category, MimeCategory::Video);
}

#[test]
fn pdf_returns_document() {
    let entry = lookup_extension("pdf").expect("pdf must be in table");
    assert_eq!(entry.mime_type, "application/pdf");
    assert_eq!(entry.category, MimeCategory::Document);
}

#[test]
fn zip_returns_archive() {
    // zip is in AMBIGUOUS_EXTENSIONS and thus NOT in the table.
    // detect_from_path handles it through new_mime_guess instead.
    assert!(lookup_extension("zip").is_none());
}

#[test]
fn unknown_extension_returns_none() {
    assert!(lookup_extension("zzz_unknown").is_none());
}

// ── Integration: detector uses table for code files ──────────────────────────

#[test]
fn detector_uses_table_for_py() {
    use std::path::Path;
    let info = MimeDetector::detect_from_path(Path::new("script.py"));
    assert_eq!(info.mime_type, "text/x-python");
    assert_eq!(info.category, MimeCategory::Text);
}

#[test]
fn detector_uses_table_for_rs() {
    use std::path::Path;
    let info = MimeDetector::detect_from_path(Path::new("main.rs"));
    assert_eq!(info.mime_type, "text/x-rust");
}
