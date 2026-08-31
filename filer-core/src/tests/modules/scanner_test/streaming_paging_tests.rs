use std::time::Duration;

use super::*;
use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::scan::scanner::{ScanCommand, Scanner};
use crate::pipeline::{FilterConfig, PipelineConfig};
use flume::Receiver;

const SCAN_TIMEOUT: Duration = Duration::from_millis(2000);
const LARGE_DIRECTORY: usize = 10_000;
const PAGE_SIZE: usize = 256;

fn streaming_pipeline() -> PipelineConfig {
    PipelineConfig {
        sort: None,
        filter: None,
        group: None,
    }
}

/// Names ordered so provider order is also readable in a failure message.
fn indexed_name(index: usize) -> String {
    format!("entry-{index:05}.txt")
}

fn large_provider(path: &str, count: usize) -> MockProvider {
    let provider = MockProvider::streaming();
    for index in 0..count {
        provider.add_file(make_file(&indexed_name(index), path, index as u64, false));
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

#[tokio::test]
async fn test_first_page_arrives_before_the_provider_reaches_end_of_directory() {
    let path = "/tmp/streaming-first-page";
    let provider = large_provider(path, LARGE_DIRECTORY);
    let stats_source = provider.clone();
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page(PAGE_SIZE),
    );
    let (groups, page) = wait_for_page(&evt_rx, session).await;

    assert_eq!(page.page_count, PAGE_SIZE);
    assert!(!page.complete);
    let stats = stats_source.stream_stats();
    assert!(
        stats.rows_yielded >= PAGE_SIZE,
        "the page must be served by the resumable walk, which yielded {} rows",
        stats.rows_yielded
    );
    assert!(
        !stats.reached_end,
        "the first page must be delivered before the walk reaches the end of the directory"
    );
    assert!(
        stats.rows_yielded < LARGE_DIRECTORY,
        "the first page pulled {} of {LARGE_DIRECTORY} rows",
        stats.rows_yielded
    );
    assert_eq!(page_names(&groups).len(), PAGE_SIZE);
}

#[tokio::test]
async fn test_first_page_pulls_only_the_page_and_its_lookahead() {
    let path = "/tmp/streaming-page-cost";
    let provider = large_provider(path, LARGE_DIRECTORY);
    let stats_source = provider.clone();
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page(PAGE_SIZE),
    );
    wait_for_page(&evt_rx, session).await;

    // One row past the page proves a continuation is warranted.
    assert_eq!(stats_source.stream_stats().rows_yielded, PAGE_SIZE + 1);
}

#[tokio::test]
async fn test_continuation_costs_one_page_and_never_replays_rows() {
    let path = "/tmp/streaming-continuation";
    let provider = large_provider(path, LARGE_DIRECTORY);
    let stats_source = provider.clone();
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page(PAGE_SIZE),
    );
    let (first_groups, first_page) = wait_for_page(&evt_rx, session).await;
    let after_first = stats_source.stream_stats().rows_yielded;
    let cursor = first_page
        .next_cursor
        .expect("a large directory should continue");

    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page_after(PAGE_SIZE, cursor),
    );
    let (second_groups, second_page) = wait_for_page(&evt_rx, session).await;
    let second_cost = stats_source.stream_stats().rows_yielded - after_first;

    assert!(
        (1..=PAGE_SIZE).contains(&second_cost),
        "a continuation pulled {second_cost} rows for a page of {PAGE_SIZE}"
    );
    assert_eq!(second_page.start_index, PAGE_SIZE);
    assert_eq!(second_page.loaded_count, PAGE_SIZE * 2);

    let first_names = page_names(&first_groups);
    let second_names = page_names(&second_groups);
    assert_eq!(first_names[0], indexed_name(0));
    assert_eq!(second_names[0], indexed_name(PAGE_SIZE));
    assert!(
        second_names.iter().all(|name| !first_names.contains(name)),
        "a continuation must not replay rows from the previous page"
    );
}

#[tokio::test]
async fn test_streaming_chain_reports_a_total_only_once_the_walk_ends() {
    let path = "/tmp/streaming-total";
    let provider = large_provider(path, 3);
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page(2),
    );
    let (_, first_page) = wait_for_page(&evt_rx, session).await;
    assert_eq!(
        first_page.total_count, None,
        "a partial streaming page cannot know the directory total"
    );
    let cursor = first_page.next_cursor.expect("three rows should continue");

    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page_after(2, cursor),
    );
    let (_, second_page) = wait_for_page(&evt_rx, session).await;

    assert!(second_page.complete);
    assert_eq!(second_page.total_count, Some(3));
}

#[tokio::test]
async fn test_streaming_filter_keeps_provider_order_and_completes_a_sparse_page() {
    let path = "/tmp/streaming-sparse";
    let provider = MockProvider::streaming();
    for index in 0..300 {
        provider.add_file(make_file(&format!("skip-{index}.txt"), path, index, false));
    }
    provider.add_file(make_file("late.rs", path, 500, false));
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let pipeline =
        PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        pipeline,
        crate::DirectoryLoadOptions::page(1),
    );
    let (groups, page) = wait_for_page(&evt_rx, session).await;

    assert_eq!(page_names(&groups), vec!["late.rs".to_string()]);
    assert!(
        page.complete,
        "a sparse filter must not report a continuation it cannot fill"
    );
}

#[tokio::test]
async fn test_terminal_page_releases_the_provider_walk() {
    let path = "/tmp/streaming-release";
    let provider = large_provider(path, 4);
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page(2),
    );
    let (_, first_page) = wait_for_page(&evt_rx, session).await;
    let cursor = first_page.next_cursor.expect("four rows should continue");

    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page_after(2, cursor.clone()),
    );
    let (_, second_page) = wait_for_page(&evt_rx, session).await;
    assert!(second_page.complete);
    assert_eq!(second_page.next_cursor, None);

    // The consumed cursor is gone, so the walk it held cannot be revived.
    request_page(
        &cmd_tx,
        path,
        session,
        streaming_pipeline(),
        crate::DirectoryLoadOptions::page_after(2, cursor),
    );
    let deadline = tokio::time::Instant::now() + SCAN_TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(Event::Error { .. })) => break,
            Ok(Ok(Event::DirectoryPageLoaded { session: s, .. })) if s == session => {
                panic!("a consumed cursor must not serve another page")
            }
            Ok(Ok(_)) => {}
            _ => panic!("timed out waiting for the expired cursor error"),
        }
    }
}

#[tokio::test]
async fn test_streaming_pages_keep_provider_order_while_an_explicit_sort_reorders() {
    let path = "/tmp/streaming-order";
    let names = ["z.rs", "a.rs", "m.rs"];
    let provider = MockProvider::streaming();
    for (index, name) in names.iter().enumerate() {
        provider.add_file(make_file(name, path, index as u64, false));
    }
    let sorted_provider = provider.clone();
    let (cmd_tx, evt_rx) = spawn_scanner(provider);

    // Without an explicit sort the pipeline adds no ordering stage, so a
    // streaming page must present rows the way the provider walked them.
    let filtered =
        PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()]));
    let session = SessionId::new();
    request_page(
        &cmd_tx,
        path,
        session,
        filtered,
        crate::DirectoryLoadOptions::page(3),
    );
    let (groups, page) = wait_for_page(&evt_rx, session).await;

    assert_eq!(page_names(&groups), vec!["z.rs", "a.rs", "m.rs"]);
    assert!(page.complete);

    let (sorted_tx, sorted_rx) = spawn_scanner(sorted_provider);
    let sorted_session = SessionId::new();
    request_page(
        &sorted_tx,
        path,
        sorted_session,
        PipelineConfig::with_default_sort(),
        crate::DirectoryLoadOptions::page(3),
    );
    let (sorted_groups, _) = wait_for_page(&sorted_rx, sorted_session).await;

    assert_eq!(page_names(&sorted_groups), vec!["a.rs", "m.rs", "z.rs"]);
}

#[tokio::test]
async fn test_local_provider_pages_a_large_directory_without_walking_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    for index in 0..600 {
        std::fs::write(path.join(indexed_name(index)), b"entry").unwrap();
    }

    let registry = NodeRegistry::new();
    let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();
    let scanner = Scanner::new(
        cmd_rx,
        evt_tx,
        Arc::new(crate::vfs::local::LocalFs::new()),
        registry,
    );
    tokio::spawn(async move { scanner.run().await });

    let session = SessionId::new();
    cmd_tx
        .send(ScanCommand::ScanLocation {
            location: location_ref(path.clone()),
            session,
            pipeline: streaming_pipeline(),
            load: crate::DirectoryLoadOptions::page(PAGE_SIZE),
            request: RequestId::new(),
        })
        .unwrap();
    let (groups, page) = wait_for_page(&evt_rx, session).await;

    assert_eq!(page.page_count, PAGE_SIZE);
    assert_eq!(page_names(&groups).len(), PAGE_SIZE);
    assert!(!page.complete);
    // Only a walk that reached the end could count the directory, so an unknown
    // total is what proves the real provider streamed this page.
    assert_eq!(page.total_count, None);
}
