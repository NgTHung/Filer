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

