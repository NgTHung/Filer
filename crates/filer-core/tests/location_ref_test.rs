use std::path::PathBuf;

use filer_core::{Location, LocationDescriptor, LocationRef};

#[test]
fn local_location_ref_preserves_local_location_identity() {
    let path = PathBuf::from("/tmp/example.txt");
    let location = Location::local(&path);

    let reference = LocationRef::local(path);

    assert_eq!(reference, LocationRef::from_location(&location));
    assert_eq!(reference.identity(), location.id());
}

#[test]
fn borrowed_location_converts_to_full_reference() {
    let location = Location::new(
        LocationDescriptor::provider_profile("sftp", "work", "/home/me/bundle.zip")
            .archive_member("src/main.rs")
            .with_display_path("Work sources"),
    );

    let reference = LocationRef::from(&location);

    assert_eq!(reference, LocationRef::from_location(&location));
    assert!(matches!(reference, LocationRef::Full { .. }));
}
