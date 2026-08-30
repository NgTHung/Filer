//! Tests for model layer

use crate::model::location::{Location, LocationDescriptor};
use crate::model::node::NodeEntry;
use std::path::PathBuf;

#[cfg(unix)]
fn non_utf8_path() -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    PathBuf::from(OsString::from_vec(b"bad-\xFF-name.txt".to_vec()))
}

#[cfg(windows)]
fn non_utf8_path() -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    PathBuf::from(OsString::from_wide(&[
        0x0062, 0x0061, 0x0064, 0x002D, 0xD800, 0x002E, 0x0074, 0x0078, 0x0074,
    ]))
}

#[test]
fn test_location_id_from_path() {
    let path = PathBuf::from("/home/user/test.txt");
    let location = Location::local(path.clone());

    let same_location = Location::local(path);
    assert_eq!(location.id(), same_location.id());
}

#[test]
fn test_location_id_from_non_utf8_path_is_stable() {
    let path = non_utf8_path();
    assert!(path.to_str().is_none());

    let location = Location::local(path.clone());
    let same_location = Location::local(path);

    assert_eq!(location.id(), same_location.id());
}

#[test]
fn test_file_node_from_metadata_accepts_non_utf8_path() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let metadata = std::fs::metadata(source).unwrap();
    let path = non_utf8_path();

    let entry = NodeEntry::from_metadata(metadata, path.clone()).unwrap();

    assert_eq!(
        entry.location.descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn test_file_node_from_dir_entry_accepts_non_utf8_path() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let file_type = std::fs::metadata(source).unwrap().file_type();
    let path = non_utf8_path();

    let entry = NodeEntry::from_dir_entry(path.clone(), file_type);

    assert_eq!(
        entry.location.descriptor(),
        Some(&LocationDescriptor::local(path))
    );
}

#[test]
fn test_location_id_different_paths() {
    let path1 = PathBuf::from("/home/user/test.txt");
    let path2 = PathBuf::from("/home/user/other.txt");

    let first = Location::local(path1);
    let second = Location::local(path2);

    assert_ne!(first.id(), second.id());
}

#[test]
fn test_node_entry_is_dir() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let nonexistent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("no.md");
    let f1 = NodeEntry::from_path(dir);
    let f2 = NodeEntry::from_path(file);
    let f3 = NodeEntry::from_path(nonexistent);
    assert_eq!(f1.is_ok(), true);
    assert_eq!(f2.is_ok(), true);
    assert_eq!(f3.is_ok(), false);
    let u1 = f1.unwrap();
    let u2 = f2.unwrap();
    assert_eq!(u1.is_dir(), true);
    assert_eq!(u2.is_file(), true);
}

#[test]
fn test_node_entry_extension() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let f1 = NodeEntry::from_path(dir).unwrap();
    let f2 = NodeEntry::from_path(file).unwrap();
    assert_eq!(f1.extension(), None);
    assert_eq!(f2.extension(), Some("toml"));
}
