use std::time::Duration;

use super::*;
use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::scan::scanner::{ScanCommand, Scanner};
use crate::pipeline::sort::{SortField, SortOrder};
use crate::pipeline::{FilterConfig, PipelineConfig};
use flume::Receiver;

const SCAN_TIMEOUT: Duration = Duration::from_millis(2000);

/// Reverse-ordered names so provider order and comparator order differ, which
/// makes an accidental provider-order page obvious.
fn reversed_name(index: usize) -> String {
    format!("entry-{:05}.txt", 9_999 - index)
}

fn reversed_provider(path: &str, count: usize) -> MockProvider {
    let provider = MockProvider::new();
    for index in 0..count {
        provider.add_file(make_file(&reversed_name(index), path, index as u64, false));
    }
    provider
}

fn spawn_scanner(provider: MockProvider) -> (flume::Sender<ScanCommand>, Receiver<Event>) {
    let registry = NodeRegistry::new();
    let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();
    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), registry);
    tokio::spawn(async move { scanner.run().await });
    (cmd_tx, evt_rx)
}

async fn wait_for_page(
    evt_rx: &Receiver<Event>,
    session: SessionId,
) -> (crate::pipeline::GroupedEntries, crate::DirectoryPageState) {
    let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(Event::DirectoryPageLoaded {
                session: s,
                groups,
                page,
                ..
            })) if s == session => return (groups, page),
            Ok(Ok(_)) => {}
            _ => panic!("timed out or channel closed waiting for DirectoryPageLoaded"),
        }
    }
}

fn page_names(groups: &crate::pipeline::GroupedEntries) -> Vec<String> {
    groups
        .groups
        .iter()
        .flat_map(|group| group.nodes.iter().map(|node| node.name.clone()))
        .collect()
}

fn request_page(
    cmd_tx: &flume::Sender<ScanCommand>,
    path: &str,
    session: SessionId,
    pipeline: PipelineConfig,
    load: crate::DirectoryLoadOptions,
) {
    cmd_tx
        .send(ScanCommand::ScanLocation {
            location: location_ref(PathBuf::from(path)),
            session,
            pipeline,
            load,
            request: RequestId::new(),
        })
        .unwrap();
}

fn sorted_pipeline() -> PipelineConfig {
    PipelineConfig::default().sort(SortField::Name, SortOrder::Ascending, true)
}

#[tokio::test]
async fn test_ordered_continuation_serves_a_page_without_another_provider_walk() {
    let path = "/tmp/ordered-retained";
    let provider = reversed_provider(path, 1_000);
    let calls = provider.clone();
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        sorted_pipeline(),
        crate::DirectoryLoadOptions::page(10),
    );
    let (first_groups, first_page) = wait_for_page(&evt_rx, session).await;
    let walk_calls = calls.get_page_calls().len();
    assert!(
        walk_calls > 0,
        "an ordered first page must walk the directory"
    );
    let cursor = first_page.next_cursor.expect("1000 rows should continue");

    request_page(
        &cmd_tx,
        path,
        session,
        sorted_pipeline(),
        crate::DirectoryLoadOptions::page_after(10, cursor),
    );
    let (second_groups, second_page) = wait_for_page(&evt_rx, session).await;

    assert_eq!(
        calls.get_page_calls().len(),
        walk_calls,
        "an ordered continuation must serve its page from retained rows"
    );
    assert_eq!(second_page.start_index, 10);
    assert_eq!(page_names(&first_groups)[0], reversed_name(999));
    assert_eq!(page_names(&second_groups)[0], reversed_name(989));
}

#[tokio::test]
async fn test_ordered_pages_keep_comparator_order_across_the_retained_tail() {
    let path = "/tmp/ordered-sequence";
    let provider = reversed_provider(path, 60);
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    let mut seen: Vec<String> = Vec::new();
    let mut load = crate::DirectoryLoadOptions::page(10);
    loop {
        request_page(&cmd_tx, path, session, sorted_pipeline(), load);
        let (groups, page) = wait_for_page(&evt_rx, session).await;
        seen.extend(page_names(&groups));
        match page.next_cursor {
            Some(cursor) => load = crate::DirectoryLoadOptions::page_after(10, cursor),
            None => break,
        }
    }

    let mut expected: Vec<String> = (0..60).map(reversed_name).collect();
    expected.sort();
    assert_eq!(seen, expected);
}

#[tokio::test]
async fn test_ordered_chain_completes_correctly_when_retention_is_unavailable() {
    let path = "/tmp/ordered-no-retention";
    let provider = reversed_provider(path, 40);
    let calls = provider.clone();
    let sessions = crate::modules::scan::paging::PagingSessions::with_limits(8, 0);
    let owner = SessionId::new();
    let pipeline = sorted_pipeline();
    let cx = crate::ProviderCx::none();

    let mut seen: Vec<String> = Vec::new();
    let mut cursor = None;
    loop {
        let request = DirectoryPageRequest {
            listing: ListingOptions::fast(),
            limit: 10,
            cursor,
        };
        let PageLoad::Page(page) = sessions
            .load_provider(&calls, Path::new(path), owner, request, &pipeline, &cx)
            .await
            .expect("page should load")
        else {
            panic!("page load was cancelled");
        };
        seen.extend(page.entries.iter().map(|entry| entry.name.clone()));
        match page.state.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    let mut expected: Vec<String> = (0..40).map(reversed_name).collect();
    expected.sort();
    assert_eq!(
        seen, expected,
        "a chain that cannot retain rows must still page correctly"
    );
}

#[tokio::test]
async fn test_retained_rows_are_released_when_the_owner_session_is_cleared() {
    let path = "/tmp/ordered-release";
    let provider = reversed_provider(path, 100);
    let sessions = crate::modules::scan::paging::PagingSessions::new();
    let owner = SessionId::new();
    let pipeline = sorted_pipeline();
    let cx = crate::ProviderCx::none();

    let PageLoad::Page(page) = sessions
        .load_provider(
            &provider,
            Path::new(path),
            owner,
            DirectoryPageRequest {
                listing: ListingOptions::fast(),
                limit: 10,
                cursor: None,
            },
            &pipeline,
            &cx,
        )
        .await
        .expect("page should load")
    else {
        panic!("page load was cancelled");
    };
    assert!(page.state.next_cursor.is_some());
    assert!(
        sessions.retained_rows() > 0,
        "an ordered chain should retain its remaining rows"
    );

    sessions.clear_session(owner);

    assert_eq!(sessions.len(), 0);
    assert_eq!(
        sessions.retained_rows(),
        0,
        "clearing a session must release the rows it retained"
    );
}

#[tokio::test]
async fn test_retention_budget_bounds_rows_across_sessions() {
    let path = "/tmp/ordered-budget";
    let provider = reversed_provider(path, 200);
    let budget = 50;
    let sessions = crate::modules::scan::paging::PagingSessions::with_limits(16, budget);
    let pipeline = sorted_pipeline();
    let cx = crate::ProviderCx::none();

    for _ in 0..8 {
        let owner = SessionId::new();
        let PageLoad::Page(page) = sessions
            .load_provider(
                &provider,
                Path::new(path),
                owner,
                DirectoryPageRequest {
                    listing: ListingOptions::fast(),
                    limit: 5,
                    cursor: None,
                },
                &pipeline,
                &cx,
            )
            .await
            .expect("page should load")
        else {
            panic!("page load was cancelled");
        };
        assert!(page.state.next_cursor.is_some());
    }

    assert!(
        sessions.retained_rows() <= budget,
        "retained rows {} exceeded the budget {budget}",
        sessions.retained_rows()
    );
}

#[tokio::test]
async fn test_snapshot_only_filter_pages_through_a_full_walk_in_comparator_order() {
    let path = "/tmp/snapshot-only";
    let provider = MockProvider::new();
    for index in 0..30 {
        provider.add_file(make_file(&reversed_name(index), path, index as u64, false));
    }
    let calls = provider.clone();
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    // A size-bounded filter cannot be applied incrementally, so this chain must
    // stay on the walked path rather than claim streaming behavior. Whether the
    // size predicate itself narrows the result is CORE-017's contract, not this
    // test's claim.
    let pipeline = PipelineConfig::default().filter(FilterConfig {
        min_size: Some(0),
        ..Default::default()
    });
    assert_eq!(
        pipeline.paging_mode(),
        crate::pipeline::PipelinePagingMode::SnapshotOnly
    );

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        pipeline.clone(),
        crate::DirectoryLoadOptions::page(10),
    );
    let (groups, page) = wait_for_page(&evt_rx, session).await;

    assert!(
        !calls.get_page_calls().is_empty(),
        "a snapshot-only chain must walk the provider"
    );
    assert_eq!(page.total_count, Some(30));
    let names = page_names(&groups);
    let mut expected: Vec<String> = (0..30).map(reversed_name).collect();
    expected.sort();
    assert_eq!(names, expected[..10]);
}

/// Names whose natural order differs from byte order in case and digit runs,
/// listed in natural order.
fn natural_names(count: usize) -> Vec<String> {
    (0..count)
        .flat_map(|index| [format!("File{index}.txt"), format!("file{index}.txt")])
        .collect()
}

fn natural_provider(path: &str, count: usize) -> MockProvider {
    fill_natural(MockProvider::new(), path, count)
}

fn fill_natural(provider: MockProvider, path: &str, count: usize) -> MockProvider {
    for (size, name) in natural_names(count).iter().rev().enumerate() {
        provider.add_file(make_file(name, path, size as u64, false));
    }
    provider
}

async fn walk_unretained_chain(
    provider: &MockProvider,
    path: &str,
    pipeline: &PipelineConfig,
    limit: usize,
) -> Vec<String> {
    let sessions = crate::modules::scan::paging::PagingSessions::with_limits(8, 0);
    let owner = SessionId::new();
    let cx = crate::ProviderCx::none();

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let request = DirectoryPageRequest {
            listing: ListingOptions::fast(),
            limit,
            cursor,
        };
        let PageLoad::Page(page) = sessions
            .load_provider(provider, Path::new(path), owner, request, pipeline, &cx)
            .await
            .expect("page should load")
        else {
            panic!("page load was cancelled");
        };
        seen.extend(page.entries.iter().map(|entry| entry.name.clone()));
        match page.state.next_cursor {
            Some(next) => cursor = Some(next),
            None => return seen,
        }
    }
}

#[tokio::test]
async fn test_ordered_pages_keep_natural_name_order_across_the_retained_tail() {
    let path = "/tmp/ordered-natural-retained";
    let (cmd_tx, evt_rx) = spawn_scanner(natural_provider(path, 20));

    let session = SessionId::new();
    let mut seen: Vec<String> = Vec::new();
    let mut load = crate::DirectoryLoadOptions::page(7);
    loop {
        request_page(&cmd_tx, path, session, sorted_pipeline(), load);
        let (groups, page) = wait_for_page(&evt_rx, session).await;
        seen.extend(page_names(&groups));
        match page.next_cursor {
            Some(cursor) => load = crate::DirectoryLoadOptions::page_after(7, cursor),
            None => break,
        }
    }

    assert_eq!(seen, natural_names(20));
}

#[tokio::test]
async fn test_keyset_rewalk_keeps_natural_name_order_without_retention() {
    let path = "/tmp/ordered-natural-keyset";
    let seen = walk_unretained_chain(&natural_provider(path, 20), path, &sorted_pipeline(), 7).await;

    assert_eq!(
        seen,
        natural_names(20),
        "a keyset rewalk must resume after the boundary in natural name order"
    );
}

#[tokio::test]
async fn test_keyset_rewalk_reverses_natural_name_order_when_descending() {
    let pipeline = PipelineConfig::default().sort(SortField::Name, SortOrder::Descending, true);
    let path = "/tmp/ordered-natural-descending";
    let seen = walk_unretained_chain(&natural_provider(path, 20), path, &pipeline, 7).await;

    let mut expected = natural_names(20);
    expected.reverse();
    assert_eq!(seen, expected);
}

#[tokio::test]
async fn test_sorted_page_load_reports_cancellation() {
    let path = "/tmp/ordered-natural-cancel";
    for provider in [MockProvider::new(), MockProvider::streaming()] {
        let provider = fill_natural(provider, path, 20);
        let sessions = crate::modules::scan::paging::PagingSessions::new();
        let cancel = crate::CancelSignal::new();
        cancel.cancel();
        let cx = crate::ProviderCx::with_cancel(&cancel);

        let load = sessions
            .load_provider(
                &provider,
                Path::new(path),
                SessionId::new(),
                DirectoryPageRequest {
                    listing: ListingOptions::fast(),
                    limit: 7,
                    cursor: None,
                },
                &sorted_pipeline(),
                &cx,
            )
            .await
            .expect("a cancelled load should not fail");

        assert!(matches!(load, PageLoad::Cancelled));
        assert_eq!(sessions.retained_rows(), 0);
    }
}

#[tokio::test]
async fn test_ordered_walk_reads_a_listing_stream_once_without_provider_pages() {
    let path = "/tmp/ordered-stream-walk";
    let provider = fill_natural(MockProvider::streaming(), path, 500);
    let (cmd_tx, evt_rx) = spawn_scanner(provider.clone());

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        sorted_pipeline(),
        crate::DirectoryLoadOptions::page(10),
    );
    let (groups, page) = wait_for_page(&evt_rx, session).await;

    let stats = provider.stream_stats();
    assert_eq!(stats.rows_yielded, 1_000, "the walk should read each row once");
    assert!(stats.reached_end);
    assert!(provider.get_page_calls().is_empty());
    assert!(provider.get_list_calls().is_empty());
    assert_eq!(page.total_count, Some(1_000));
    assert_eq!(page_names(&groups), natural_names(5));
}

#[tokio::test]
async fn test_grouped_keyset_rewalk_matches_the_flat_pipeline() {
    let path = "/tmp/ordered-grouped-keyset";
    let names = [
        "b10.rs", "B2.md", "a1.rs", "A10.md", "notes", "Notes2", "c01.rs", "c1.md", "z.txt",
        "Y9.txt", "y10.txt", "readme",
    ];
    let pipeline = PipelineConfig::default()
        .sort(SortField::Name, SortOrder::Ascending, true)
        .group_by(crate::pipeline::GroupBy::Extension);
    let expected: Vec<String> = crate::pipeline::Pipeline::from_config(&pipeline)
        .execute_flat(
            names
                .iter()
                .map(|name| make_file(name, path, 0, false))
                .collect(),
        )
        .into_iter()
        .map(|entry| entry.name)
        .collect();

    for provider in [MockProvider::new(), MockProvider::streaming()] {
        for name in names {
            provider.add_file(make_file(name, path, 0, false));
        }
        let seen = walk_unretained_chain(&provider, path, &pipeline, 3).await;
        assert_eq!(seen, expected);
    }
}
