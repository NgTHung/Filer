#[cfg(test)]
mod segmented_location_tests {
    use super::*;
    use crate::model::location::{LocationDescriptor, LocationSegment};
    use crate::model::node::NodeKind;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn nested_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn segmented_resolver_lists_zip_root_with_target_locations() {
        let (fs, dir) = local_fs();
        let archive = dir.path().join("bundle.zip");
        write_zip(
            &archive,
            &[("src/main.rs", b"fn main() {}"), ("README.md", b"readme")],
        );
        let location = LocationDescriptor::local(&archive).archive_member("");

        let entries = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        let src = entries.iter().find(|entry| entry.name == "src").unwrap();
        assert!(matches!(src.kind, NodeKind::Directory { .. }));
        assert!(src.capabilities.read);
        assert!(src.capabilities.navigate);
        assert_eq!(
            src.location.descriptor(),
            Some(&LocationDescriptor::local(&archive).archive_member("src"))
        );

        let readme = entries
            .iter()
            .find(|entry| entry.name == "README.md")
            .unwrap();
        assert!(matches!(readme.kind, NodeKind::File { .. }));
        assert!(readme.capabilities.read);
        assert!(!readme.capabilities.navigate);
        assert_eq!(
            readme.location.descriptor(),
            Some(&LocationDescriptor::local(&archive).archive_member("README.md"))
        );
    }

    #[tokio::test]
    async fn segmented_resolver_rejects_nested_zip_layers_until_member_reads_exist() {
        let (fs, dir) = local_fs();
        let archive = dir.path().join("outer.zip");
        let inner = nested_zip_bytes(&[("inner.txt", b"inside")]);
        write_zip(&archive, &[("nested.zip", &inner)]);
        let location = LocationDescriptor::local(&archive)
            .archive_member("nested.zip")
            .archive_member("");

        let error = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::LocationSegmentedUnsupported);
    }

    #[tokio::test]
    async fn segmented_resolver_rejects_virtual_segments_as_structured_error() {
        let (fs, dir) = local_fs();
        let archive = dir.path().join("bundle.zip");
        write_zip(&archive, &[("README.md", b"readme")]);
        let location = LocationDescriptor::local(&archive).with_segment(LocationSegment::Virtual {
            scheme: "git".to_string(),
            path: PathBuf::from("HEAD"),
        });

        let error = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::LocationSegmentedUnsupported);
    }

    #[tokio::test]
    async fn segmented_resolver_rejects_non_local_provider_as_structured_error() {
        let fs = MockFs::new();
        let location = LocationDescriptor::provider_profile("s3", "assets", "bucket/archive.zip")
            .archive_member("");

        let error = SegmentedLocationResolver::new(&fs)
            .list(&location, &crate::ProviderCx::none())
            .await
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::UnsupportedProvider);
    }
}

