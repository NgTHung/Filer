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

