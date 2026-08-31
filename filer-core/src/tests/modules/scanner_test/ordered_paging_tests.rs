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
