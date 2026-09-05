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

    #[cfg(feature = "metadata-video")]
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

    #[cfg(feature = "metadata-video")]
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

    #[cfg(feature = "metadata-video")]
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

    #[cfg(feature = "metadata-video")]
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

