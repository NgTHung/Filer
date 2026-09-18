//! Search Module Integration Tests
//!
//! These tests exercise the full command→event pipeline for search:
//!   FilerCore (Command::Search) → Router → SearchModule → Searcher → Event::SearchResults
//!
//! The module stack used in every test:
//!   ScanModule::new(MockProvider) + SearchModule::new(MockProvider)

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

mod support;

use filer_core::model::session::SessionId;
use filer_core::modules::scan::ScanModule;
use filer_core::modules::search::SearchModule;
use filer_core::{Command, Event, FilerCore};

use support::nodes::{directory as make_dir, file as make_file};
use support::provider::MemoryProvider as MockProvider;
use support::{local_location, wait_for_search_entries};

const TIMEOUT: Duration = Duration::from_millis(3000);

fn build_core_with_search(provider: MockProvider) -> FilerCore {
    let provider = Arc::new(provider);
    let core = FilerCore::new();
    core.load(ScanModule::new(provider.clone()));
    core.load(SearchModule::new(provider));
    core
}

async fn create_session(core: &FilerCore) -> SessionId {
    let rx = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    match timeout(TIMEOUT, rx.recv_async()).await {
        Ok(Ok(Event::SessionCreated(id))) => id,
        other => panic!("expected SessionCreated, got {:?}", other),
    }
}

#[tokio::test]
async fn test_search_command_through_filer_core() {
    let provider = MockProvider::directories(true);
    provider.add_dir(
        "/root",
        vec![
            make_file("target.rs", "/root", 100),
            make_file("other.py", "/root", 200),
        ],
    );

    let core = build_core_with_search(provider);
    let session = create_session(&core).await;
    core.send(Command::Search {
        query: "target".to_string(),
        root: local_location("/root"),
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let rx = core.event_receiver();
    let matches = wait_for_search_entries(&rx, session, TIMEOUT).await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "target.rs");
    assert_eq!(matches[0].location, local_location("/root/target.rs"));
}

#[tokio::test]
async fn test_session_destroy_cancels_search() {
    let provider = MockProvider::directories(true);
    // Build a deep tree so search takes multiple scheduler quanta
    let mut path = PathBuf::from("/root");
    for i in 0..10 {
        provider.add_dir(
            path.clone(),
            vec![
                make_file(&format!("f{}.txt", i), path.to_str().unwrap(), 100),
                make_dir(&format!("d{}", i), path.to_str().unwrap()),
            ],
        );
        path = path.join(format!("d{}", i));
    }

    let core = build_core_with_search(provider);
    let session = create_session(&core).await;
    core.send(Command::Search {
        query: "f".to_string(),
        root: local_location("/root"),
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    // Immediately destroy session — should trigger search cancellation
    core.send(Command::DestroySession(session)).unwrap();

    // No crash expected; search should have been cancelled
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_search_cancel_command() {
    let provider = MockProvider::directories(true);
    provider.add_dir("/root", vec![make_file("file.txt", "/root", 100)]);

    let core = build_core_with_search(provider);
    let session = create_session(&core).await;
    core.send(Command::Search {
        query: "file".to_string(),
        root: local_location("/root"),
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    core.send(Command::CancelSearch { session }).unwrap();

    // Should not crash
    tokio::time::sleep(Duration::from_millis(200)).await;
}
