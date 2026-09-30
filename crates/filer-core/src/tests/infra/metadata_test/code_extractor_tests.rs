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
