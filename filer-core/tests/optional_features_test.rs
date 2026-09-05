#![cfg(not(feature = "metadata-archive"))]

use std::path::Path;
use std::sync::Arc;

use filer_core::{ArchiveFs, ErrorCode, FsProvider, LocalFs, ProviderCx};

#[tokio::test]
async fn archive_listing_without_feature_returns_unsupported_before_io() {
    let directory = tempfile::tempdir().expect("temporary directory should be available");
    let archive = ArchiveFs::zip(directory.path().join("missing.zip"), Arc::new(LocalFs::new()));

    let error = archive
        .list(Path::new(""), &ProviderCx::none())
        .await
        .expect_err("archive listing requires metadata-archive");

    assert_eq!(error.code(), ErrorCode::UnsupportedOperation);
}
