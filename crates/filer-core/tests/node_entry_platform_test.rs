#[test]
fn fast_listing_uses_platform_hidden_convention() {
    use std::path::PathBuf;

    use filer_core::NodeEntry;

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let file_type = std::fs::metadata(source)
        .expect("the crate manifest should be available")
        .file_type();
    let entry = NodeEntry::from_dir_entry(PathBuf::from(".gitignore"), file_type);

    #[cfg(unix)]
    assert!(entry.meta.hidden);

    #[cfg(not(unix))]
    assert!(!entry.meta.hidden);
}
