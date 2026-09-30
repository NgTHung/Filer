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

    #[cfg(feature = "metadata-image")]
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

    #[cfg(feature = "metadata-image")]
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

    #[cfg(feature = "metadata-image")]
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

    #[cfg(feature = "metadata-image")]
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

    #[cfg(feature = "metadata-image")]
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

    #[cfg(feature = "metadata-image")]
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

