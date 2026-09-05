#![cfg(feature = "preview-code")]

use std::fs;

use filer_core::services::mime::{DetectionConfidence, MimeCategory, MimeInfo};
use filer_core::services::preview::providers::CodeProvider;
use filer_core::services::preview::{PreviewData, PreviewOptions, PreviewProvider};

fn text_mime() -> MimeInfo {
    MimeInfo {
        mime_type: "text/plain".to_string(),
        category: MimeCategory::Text,
        encoding: None,
        confidence: DetectionConfidence::Definitive,
    }
}

#[tokio::test]
async fn unknown_theme_preserves_highlighted_preview_payload() {
    let directory = tempfile::tempdir().expect("temporary directory should be available");
    let path = directory.path().join("sample.rs");
    let source = "fn main() {}\n";
    fs::write(&path, source).expect("source fixture should be writable");

    let mut options = PreviewOptions::default();
    options.syntax_theme = "theme-that-does-not-exist".to_string();

    let result = CodeProvider::new()
        .generate(&path, &text_mime(), &options)
        .await
        .expect("code preview should be generated");

    match result {
        PreviewData::HighlightedText {
            content,
            language,
            theme,
            truncated,
        } => {
            assert_ne!(content, source);
            assert!(content.contains("main"));
            assert_eq!(language, "Rust");
            assert_eq!(theme, "theme-that-does-not-exist");
            assert!(!truncated);
        }
        other => panic!("expected highlighted text, got {other:?}"),
    }
}
