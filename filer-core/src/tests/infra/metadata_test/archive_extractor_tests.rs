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

