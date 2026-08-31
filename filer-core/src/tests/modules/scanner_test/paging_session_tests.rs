use crate::errors::ErrorCode;
use crate::modules::scan::paging::{PageLoad, PagingSessions};
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig as PagingPipelineConfig;
use crate::vfs::context::ProviderCx as PagingProviderCx;

fn page_request(cursor: Option<DirectoryCursor>) -> DirectoryPageRequest {
    DirectoryPageRequest {
        listing: ListingOptions::fast(),
        limit: 1,
        cursor,
    }
}

fn load_page(
    sessions: &PagingSessions,
    owner: SessionId,
    path: &Path,
    request: DirectoryPageRequest,
    pipeline: &PagingPipelineConfig,
) -> Result<PageLoad, CoreError> {
    sessions.load_cached(
        vec![
            make_file("a.txt", path.to_str().unwrap(), 10, false),
            make_file("b.txt", path.to_str().unwrap(), 20, false),
        ],
        path,
        owner,
        request,
        pipeline,
        &PagingProviderCx::none(),
    )
}

#[test]
fn paging_sessions_evict_oldest_cursor_at_capacity() {
    let sessions = PagingSessions::with_capacity(2);
    let pipeline = PagingPipelineConfig::default();
    let path = Path::new("/tmp/paging-session-capacity");
    let mut cursors = Vec::new();

    for _ in 0..3 {
        let owner = SessionId::new();
        let PageLoad::Page(page) = load_page(
            &sessions,
            owner,
            path,
            page_request(None),
            &pipeline,
        )
        .expect("first page should load")
        else {
            panic!("page load was cancelled");
        };
        cursors.push((owner, page.state.next_cursor.expect("page should continue")));
    }

    assert_eq!(sessions.len(), 2);

    let (old_owner, old_cursor) = cursors[0].clone();
    let expired = match load_page(
        &sessions,
        old_owner,
        path,
        page_request(Some(old_cursor)),
        &pipeline,
    ) {
        Ok(_) => panic!("the oldest cursor should have been evicted"),
        Err(error) => error,
    };
    assert_eq!(expired.code(), ErrorCode::InputInvalid);
    assert!(expired.to_string().contains("Expired directory paging cursor"));

    let (new_owner, new_cursor) = cursors[2].clone();
    let PageLoad::Page(page) = load_page(
        &sessions,
        new_owner,
        path,
        page_request(Some(new_cursor)),
        &pipeline,
    )
    .expect("the newest cursor should remain usable")
    else {
        panic!("page load was cancelled");
    };
    assert!(page.state.complete);
}

#[test]
fn paging_cursor_is_single_use() {
    let sessions = PagingSessions::with_capacity(2);
    let pipeline = PagingPipelineConfig::default();
    let path = Path::new("/tmp/paging-session-single-use");
    let owner = SessionId::new();
    let PageLoad::Page(first) = load_page(
        &sessions,
        owner,
        path,
        page_request(None),
        &pipeline,
    )
    .expect("first page should load")
    else {
        panic!("page load was cancelled");
    };
    let cursor = first.state.next_cursor.expect("page should continue");

    let second = load_page(
        &sessions,
        owner,
        path,
        page_request(Some(cursor.clone())),
        &pipeline,
    )
    .expect("continuation should load");
    assert!(matches!(second, PageLoad::Page(page) if page.state.complete));

    let replay = match load_page(
        &sessions,
        owner,
        path,
        page_request(Some(cursor)),
        &pipeline,
    ) {
        Ok(_) => panic!("a consumed cursor should not be replayable"),
        Err(error) => error,
    };
    assert_eq!(replay.code(), ErrorCode::InputInvalid);
    assert!(replay.to_string().contains("Expired directory paging cursor"));
}

#[test]
fn cursor_request_mismatch_does_not_consume_session() {
    let sessions = PagingSessions::with_capacity(2);
    let pipeline = PagingPipelineConfig::default();
    let path = Path::new("/tmp/paging-session-mismatch");
    let owner = SessionId::new();
    let PageLoad::Page(first) = load_page(
        &sessions,
        owner,
        path,
        page_request(None),
        &pipeline,
    )
    .expect("first page should load")
    else {
        panic!("page load was cancelled");
    };
    let cursor = first.state.next_cursor.expect("page should continue");

    let mismatch = match load_page(
        &sessions,
        owner,
        path,
        page_request(Some(cursor.clone())),
        &PagingPipelineConfig::with_default_sort(),
    ) {
        Ok(_) => panic!("a changed pipeline should be rejected"),
        Err(error) => error,
    };
    assert_eq!(mismatch.code(), ErrorCode::InputInvalid);
    assert!(mismatch
        .to_string()
        .contains("does not match the request"));

    let continuation = load_page(
        &sessions,
        owner,
        path,
        page_request(Some(cursor)),
        &pipeline,
    )
    .expect("the rejected request must not consume the session");
    assert!(matches!(continuation, PageLoad::Page(page) if page.state.complete));
}
