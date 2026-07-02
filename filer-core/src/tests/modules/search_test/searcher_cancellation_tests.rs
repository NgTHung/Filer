#[cfg(test)]
mod searcher_cancellation_tests {
    use super::*;
    use crate::model::location::{Location, LocationRef};

    #[tokio::test]
    async fn test_search_cancel_stops_search() {
        // Create a deep tree so search takes time
        let provider = MockProvider::new();
        let mut dirs = vec![];
        let mut path = PathBuf::from("/root");
        for i in 0..20 {
            let children = vec![
                MockProvider::make_file(&format!("file{}.txt", i), path.to_str().unwrap(), 100),
                MockProvider::make_dir(&format!("d{}", i), path.to_str().unwrap()),
            ];
            dirs.push((path.clone(), children));
            path = path.join(format!("d{}", i));
        }
        for (p, c) in dirs {
            provider.add_dir(p, c);
        }

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("file").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: root_id,
                session,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // Immediately cancel
        cmd_tx.send(SearchCommand::Cancel(session)).unwrap();

        // Collect events for a short window
        let events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        // Count matched files across all batches for this session
        let total_matches: usize = events
            .iter()
            .filter_map(|e| {
                if let Event::SearchResultsCompat {
                    matches,
                    session: s,
                    ..
                } = e
                {
                    if *s == session {
                        return Some(matches.len());
                    }
                }
                None
            })
            .sum();

        // yield_now() in the mock guarantees the scheduler processes the Cancel
        // command between directory listings, so the search stops well before
        // completing all 20 levels. Fewer than half the files is a reliable bound.
        assert!(
            total_matches < 10,
            "cancel should stop search well before finding all 20 files (found {})",
            total_matches
        );
    }

    #[tokio::test]
    async fn test_search_location_cancel_stops_results() {
        let provider = MockProvider::new();
        let mut dirs = vec![];
        let mut path = PathBuf::from("/root");
        for i in 0..20 {
            let children = vec![
                MockProvider::make_file(&format!("file{}.txt", i), path.to_str().unwrap(), 100),
                MockProvider::make_dir(&format!("d{}", i), path.to_str().unwrap()),
            ];
            dirs.push((path.clone(), children));
            path = path.join(format!("d{}", i));
        }
        for (p, c) in dirs {
            provider.add_dir(p, c);
        }

        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let location = Location::local("/root");
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("file").unwrap(),
                root: LocationRef::from_location(&location),
                session,
                request: RequestId::new(),
            })
            .unwrap();
        cmd_tx.send(SearchCommand::Cancel(session)).unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;
        let total_matches: usize = events
            .iter()
            .filter_map(|e| {
                if let Event::SearchResults {
                    matches,
                    session: s,
                    ..
                } = e
                    && *s == session
                {
                    return Some(matches.len());
                }
                None
            })
            .sum();

        assert!(
            total_matches < 10,
            "cancel should stop SearchLocation well before all 20 files (found {})",
            total_matches
        );
    }

    #[tokio::test]
    async fn test_cancel_one_session_doesnt_affect_other() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("target.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session1 = SessionId::new();
        let session2 = SessionId::new();

        let query1 = SearchQuery::parse("target").unwrap();
        let query2 = SearchQuery::parse("target").unwrap();

        cmd_tx
            .send(SearchCommand::Search {
                query: query1,
                root: root_id,
                session: session1,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx.send(SearchCommand::Cancel(session1)).unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query: query2,
                root: root_id,
                session: session2,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();

        // session2 should still complete
        let matches = wait_for_search_complete(&evt_rx, session2).await;
        assert_eq!(
            matches.len(),
            1,
            "cancelling session1 should not affect session2"
        );
    }

    #[tokio::test]
    async fn test_cancel_search_location_one_session_doesnt_affect_other() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("target.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session1 = SessionId::new();
        let session2 = SessionId::new();
        let location = Location::local("/root");

        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                session: session1,
                request: RequestId::new(),
            })
            .unwrap();
        cmd_tx.send(SearchCommand::Cancel(session1)).unwrap();
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                session: session2,
                request: RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_entries_complete(&evt_rx, session2).await;
        assert_eq!(
            matches.len(),
            1,
            "cancelling one SearchLocation session should not affect another"
        );
    }

    #[tokio::test]
    async fn test_stale_search_results_are_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("target.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: root_id,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: root_id,
                session,
                request: fresh_request,
            })
            .unwrap();

        let mut result_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::SearchResultsCompat {
                session: s,
                request,
                complete,
                ..
            } = event
                && s == session
            {
                result_requests.push(request);
                if request == fresh_request && complete {
                    break;
                }
            }
        }

        assert_eq!(result_requests, vec![fresh_request]);
        assert!(
            !result_requests.contains(&stale_request),
            "stale search request should not emit SearchResultsCompat"
        );
    }

    #[tokio::test]
    async fn test_stale_search_location_results_are_suppressed() {
        let provider = MockProvider::new();
        provider.set_delay_ms(50);
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("target.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/root");

        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                session,
                request: fresh_request,
            })
            .unwrap();

        let mut result_requests = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            if let Event::SearchResults {
                session: s,
                request,
                complete,
                ..
            } = event
                && s == session
            {
                result_requests.push(request);
                if request == fresh_request && complete {
                    break;
                }
            }
        }

        assert_eq!(result_requests, vec![fresh_request]);
        assert!(
            !result_requests.contains(&stale_request),
            "stale SearchLocation request should not emit SearchResults"
        );
    }
}
