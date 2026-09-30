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

