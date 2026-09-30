use std::fs::{self, FileTimes, OpenOptions};
use std::path::PathBuf;
use std::time::SystemTime;

use filer_core_benchmarks::{
    FixtureErrorCode, ValidatedManifest, prepare_fixture, prepare_fixture_from_path,
};
use serde_json::Value;
use tempfile::tempdir;

fn manifest(name: &str) -> ValidatedManifest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join(name);
    ValidatedManifest::load(path).expect("manifest should validate")
}

#[test]
fn prepares_and_verifies_flat_10k_fixture() {
    let manifest = manifest("flat-10k-v1.json");
    let parent = tempdir().expect("temporary parent");
    let target = parent.path().join("flat-10k");
    let fixture = prepare_fixture(&manifest, &target).expect("10k fixture should prepare");

    fixture.verify().expect("10k readback should verify");
    assert_eq!(fixture.root(), target.as_path());
    assert_eq!(fixture.rows().len(), 10_000);
}

#[test]
fn prepares_and_verifies_flat_100k_fixture() {
    let manifest = manifest("flat-100k-v1.json");
    let parent = tempdir().expect("temporary parent");
    let target = parent.path().join("flat-100k");
    let fixture = prepare_fixture(&manifest, &target).expect("100k fixture should prepare");

    fixture.verify().expect("100k readback should verify");
    assert_eq!(fixture.rows().len(), 100_000);
}

#[test]
fn fixture_results_do_not_depend_on_parent_root() {
    let manifest = manifest("flat-10k-v1.json");
    let parent_a = tempdir().expect("first temporary parent");
    let parent_b = tempdir().expect("second temporary parent");
    let fixture_a = prepare_fixture(&manifest, parent_a.path().join("fixture"))
        .expect("first fixture should prepare");
    let fixture_b = prepare_fixture(&manifest, parent_b.path().join("fixture"))
        .expect("second fixture should prepare");

    assert_ne!(fixture_a.root(), fixture_b.root());
    assert_eq!(fixture_a.rows(), fixture_b.rows());
    fixture_a.verify().expect("first readback should verify");
    fixture_b.verify().expect("second readback should verify");
}

#[test]
fn rejects_existing_targets_without_overwriting_them() {
    let manifest = manifest("flat-10k-v1.json");
    let parent = tempdir().expect("temporary parent");
    let target = parent.path().join("existing");
    fs::create_dir(&target).expect("existing target");
    fs::write(target.join("sentinel"), b"keep").expect("sentinel");

    let error = prepare_fixture(&manifest, &target).expect_err("existing target should reject");
    assert_eq!(error.code(), FixtureErrorCode::TargetExists);
    assert_eq!(
        fs::read(target.join("sentinel")).expect("sentinel"),
        b"keep"
    );
}

#[test]
fn rejects_corrupt_manifest_before_creating_target() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join("flat-10k-v1.json");
    let bytes = fs::read(&source).expect("manifest bytes");
    let mut value: Value = serde_json::from_slice(&bytes).expect("manifest JSON");
    value["entry_count"] = 12_345.into();
    let parent = tempdir().expect("temporary parent");
    let manifest_path = parent.path().join("corrupt.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&value).expect("manifest JSON"),
    )
    .expect("write");
    let target = parent.path().join("fixture");

    let error = prepare_fixture_from_path(&manifest_path, &target)
        .expect_err("corrupt manifest should reject");
    assert_eq!(error.code(), FixtureErrorCode::Manifest);
    assert!(!target.exists());
}

#[test]
fn readback_rejects_changed_file_metadata() {
    let manifest = manifest("flat-10k-v1.json");
    let parent = tempdir().expect("temporary parent");
    let fixture =
        prepare_fixture(&manifest, parent.path().join("fixture")).expect("fixture should prepare");
    let file = fixture.root().join("file-000001.txt");
    let handle = OpenOptions::new()
        .write(true)
        .open(&file)
        .expect("generated file");
    handle.set_len(1).expect("corrupt size");

    let error = fixture.verify().expect_err("changed size should reject");
    assert_eq!(error.code(), FixtureErrorCode::SizeMismatch);
}

#[test]
fn readback_reports_missing_root_as_io() {
    let manifest = manifest("flat-10k-v1.json");
    let parent = tempdir().expect("temporary parent");
    let fixture =
        prepare_fixture(&manifest, parent.path().join("fixture")).expect("fixture should prepare");
    fs::remove_dir_all(fixture.root()).expect("remove fixture root");

    let error = fixture.verify().expect_err("missing root should reject");
    assert_eq!(error.code(), FixtureErrorCode::Io);
}

#[test]
fn readback_rejects_extra_missing_kind_and_timestamp_changes() {
    let manifest = manifest("flat-10k-v1.json");
    let parent = tempdir().expect("temporary parent");

    let extra = prepare_fixture(&manifest, parent.path().join("extra"))
        .expect("extra fixture should prepare");
    fs::write(extra.root().join("unexpected"), b"extra").expect("extra entry");
    assert_eq!(
        extra
            .verify()
            .expect_err("extra entry should reject")
            .code(),
        FixtureErrorCode::ExtraEntry
    );

    let missing = prepare_fixture(&manifest, parent.path().join("missing"))
        .expect("missing fixture should prepare");
    fs::remove_file(missing.root().join("file-000001.txt")).expect("remove expected file");
    assert_eq!(
        missing
            .verify()
            .expect_err("missing entry should reject")
            .code(),
        FixtureErrorCode::MissingEntry
    );

    let wrong_kind = prepare_fixture(&manifest, parent.path().join("kind"))
        .expect("kind fixture should prepare");
    let kind_path = wrong_kind.root().join("file-000001.txt");
    fs::remove_file(&kind_path).expect("remove file");
    fs::create_dir(&kind_path).expect("replace file with directory");
    assert_eq!(
        wrong_kind
            .verify()
            .expect_err("kind change should reject")
            .code(),
        FixtureErrorCode::KindMismatch
    );

    let timestamp = prepare_fixture(&manifest, parent.path().join("timestamp"))
        .expect("timestamp fixture should prepare");
    let file = OpenOptions::new()
        .write(true)
        .open(timestamp.root().join("file-000001.txt"))
        .expect("generated file");
    file.set_times(FileTimes::new().set_modified(SystemTime::now()))
        .expect("change timestamp");
    assert_eq!(
        timestamp
            .verify()
            .expect_err("timestamp change should reject")
            .code(),
        FixtureErrorCode::TimestampMismatch
    );
}

#[test]
fn explicit_fixture_close_removes_owned_directory() {
    let manifest = manifest("flat-10k-v1.json");
    let parent = tempdir().expect("temporary parent");
    let target = parent.path().join("fixture");
    let fixture = prepare_fixture(&manifest, &target).expect("fixture should prepare");

    fixture.close().expect("owned cleanup should succeed");
    assert!(!target.exists());
}
