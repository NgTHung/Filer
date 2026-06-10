use crate::model::directory::{
    DEFAULT_DIRECTORY_PAGE_SIZE, DirectoryCursor, DirectoryLoadMode, DirectoryLoadOptions,
    DirectoryLoadState, DirectoryPageState,
};
use crate::vfs::provider::ListingOptions;

#[test]
fn test_directory_load_options_default_is_fast_first_page() {
    let options = DirectoryLoadOptions::default();

    assert_eq!(options.listing, ListingOptions::fast());
    assert_eq!(
        options.mode,
        DirectoryLoadMode::Page {
            limit: DEFAULT_DIRECTORY_PAGE_SIZE,
            cursor: None
        }
    );
    assert!(options.is_paged());
}

#[test]
fn test_directory_load_options_unbounded_uses_snapshot_mode() {
    let options = DirectoryLoadOptions::unbounded(ListingOptions::metadata());

    assert_eq!(options.listing, ListingOptions::metadata());
    assert_eq!(options.mode, DirectoryLoadMode::Snapshot { limit: None });
    assert!(!options.is_bounded());
    assert!(!options.is_paged());
}

#[test]
fn test_directory_load_options_bounded_uses_snapshot_limit() {
    let options = DirectoryLoadOptions::bounded(25);

    assert_eq!(options.listing, ListingOptions::fast());
    assert_eq!(
        options.mode,
        DirectoryLoadMode::Snapshot { limit: Some(25) }
    );
    assert!(options.is_bounded());
}

#[test]
fn test_directory_load_options_page_after_sets_cursor() {
    let cursor = DirectoryCursor("next".into());
    let options = DirectoryLoadOptions::page_after(50, cursor.clone());

    assert_eq!(
        options.mode,
        DirectoryLoadMode::Page {
            limit: 50,
            cursor: Some(cursor)
        }
    );
    assert!(options.is_paged());
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

#[test]
fn test_directory_page_state_marks_partial_and_complete_pages() {
    let cursor = DirectoryCursor("cursor".into());
    assert_eq!(
        DirectoryPageState::partial(25, Some(100), cursor.clone()),
        DirectoryPageState {
            page_count: 25,
            total_count: Some(100),
            next_cursor: Some(cursor),
            complete: false,
            start_index: 0,
            loaded_count: 25,
        }
    );
    assert_eq!(
        DirectoryPageState::complete(10, Some(35)),
        DirectoryPageState {
            page_count: 10,
            total_count: Some(35),
            next_cursor: None,
            complete: true,
            start_index: 0,
            loaded_count: 10,
        }
    );
}

#[test]
fn test_directory_page_state_exposes_virtualization_window() {
    let state =
        DirectoryPageState::partial(25, Some(100), DirectoryCursor("next".into())).with_window(50);

    assert_eq!(state.start_index, 50);
    assert_eq!(state.loaded_count, 75);
}

#[test]
fn test_directory_page_state_deserializes_legacy_payload_without_window() {
    let state: DirectoryPageState = serde_json::from_str(
        r#"{"page_count":2,"total_count":3,"next_cursor":null,"complete":false}"#,
    )
    .unwrap();

    assert_eq!(state.start_index, 0);
    assert_eq!(state.loaded_count, 0);
}
