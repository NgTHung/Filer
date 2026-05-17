use crate::model::directory::{DirectoryLoadOptions, DirectoryLoadState};
use crate::vfs::provider::ListingOptions;

#[test]
fn test_directory_load_options_default_is_unbounded_fast() {
    let options = DirectoryLoadOptions::default();

    assert_eq!(options.listing, ListingOptions::fast());
    assert_eq!(options.limit, None);
    assert!(!options.is_bounded());
}

#[test]
fn test_directory_load_options_bounded_uses_fast_listing() {
    let options = DirectoryLoadOptions::bounded(25);

    assert_eq!(options.listing, ListingOptions::fast());
    assert_eq!(options.limit, Some(25));
    assert!(options.is_bounded());
}

#[test]
fn test_directory_load_state_from_counts_marks_completion() {
    assert_eq!(
        DirectoryLoadState::from_counts(5, 10),
        DirectoryLoadState {
            loaded_count: 5,
            total_count: Some(10),
            complete: false,
        }
    );
    assert_eq!(
        DirectoryLoadState::from_counts(10, 10),
        DirectoryLoadState::complete(10)
    );
}
