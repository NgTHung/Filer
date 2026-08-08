#[cfg(test)]
mod searcher_cancellation_tests {
    use super::*;
    use crate::model::location::{Location, LocationRef};

    #[derive(Clone)]
    struct CleanupInterleavingProvider;

    #[async_trait]
    impl FsProvider for CleanupInterleavingProvider {
        fn scheme(&self) -> &'static str {
            "cleanup-interleaving"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                read: true,
                write: false,
                watch: false,
                search: true,
            }
        }

        async fn list(
            &self,
            path: &Path,
            _cx: &crate::ProviderCx<'_>,
        ) -> Result<Vec<FileNode>, CoreError> {
            match path.to_str() {
                Some("/stale") => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Ok(vec![MockProvider::make_file("stale.txt", "/stale", 1)])
                }
                Some("/fresh") => {
                    tokio::time::sleep(Duration::from_millis(120)).await;
                    Ok(vec![MockProvider::make_file("fresh.txt", "/fresh", 1)])
                }
                _ => Ok(vec![]),
            }
        }

        async fn read(
            &self,
            _path: &Path,
            _cx: &crate::ProviderCx<'_>,
        ) -> Result<Vec<u8>, CoreError> {
            Ok(vec![])
        }

        async fn read_range(
            &self,
            _path: &Path,
            _start: u64,
            _len: u64,
            _cx: &crate::ProviderCx<'_>,
        ) -> Result<Vec<u8>, CoreError> {
            Ok(vec![])
        }

        async fn exists(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
            Ok(true)
        }

        async fn metadata(
            &self,
            path: &Path,
            _cx: &crate::ProviderCx<'_>,
        ) -> Result<FileNode, CoreError> {
            Err(CoreError::not_found(path.to_path_buf()))
        }
    }

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
                let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let query = SearchQuery::parse("file").unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query,
                root: LocationRef::from_location(&Location::local("/root")),
                event_mode: SearchEventMode::Location,
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
                if let Event::SearchResults {
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
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let location = Location::local("/root");
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("file").unwrap(),
                root: LocationRef::from_location(&location),
                event_mode: SearchEventMode::Location,
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
    async fn test_search_rapid_reissue_then_cancel_cancels_fresh_search() {
        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let searcher = Searcher::new(
            cmd_rx,
            evt_tx,
            Arc::new(CleanupInterleavingProvider),
            registry,
        );
        tokio::spawn(async move {
            searcher.run().await;
        });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("stale").unwrap(),
                root: LocationRef::from_location(&Location::local(PathBuf::from("/stale"))),
                event_mode: SearchEventMode::Location,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("fresh").unwrap(),
                root: LocationRef::from_location(&Location::local(PathBuf::from("/fresh"))),
                event_mode: SearchEventMode::Location,
                session,
                request: fresh_request,
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        cmd_tx.send(SearchCommand::Cancel(session)).unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(180)).await;
        assert!(
            events.iter().all(|event| !matches!(
                event,
                Event::SearchResults {
                    session: s,
                    request,
                    complete: true,
                    ..
                } if *s == session && *request == fresh_request
            )),
            "fresh search completed after rapid reissue cancellation: {events:?}"
        );
    }

    #[tokio::test]
    async fn test_search_location_rapid_reissue_then_cancel_cancels_fresh_search() {
        let registry = NodeRegistry::new();
        let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
        let (evt_tx, evt_rx) = flume::unbounded::<Event>();
        let searcher = Searcher::new(
            cmd_rx,
            evt_tx,
            Arc::new(CleanupInterleavingProvider),
            registry,
        );
        tokio::spawn(async move {
            searcher.run().await;
        });

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("stale").unwrap(),
                root: LocationRef::from_location(&Location::local("/stale")),
                event_mode: SearchEventMode::Location,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("fresh").unwrap(),
                root: LocationRef::from_location(&Location::local("/fresh")),
                event_mode: SearchEventMode::Location,
                session,
                request: fresh_request,
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        cmd_tx.send(SearchCommand::Cancel(session)).unwrap();

        let events = collect_events_for(&evt_rx, Duration::from_millis(180)).await;
        assert!(
            events.iter().all(|event| !matches!(
                event,
                Event::SearchResults {
                    session: s,
                    request,
                    complete: true,
                    ..
                } if *s == session && *request == fresh_request
            )),
            "fresh SearchLocation completed after rapid reissue cancellation: {events:?}"
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
                let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session1 = SessionId::new();
        let session2 = SessionId::new();

        let query1 = SearchQuery::parse("target").unwrap();
        let query2 = SearchQuery::parse("target").unwrap();

        cmd_tx
            .send(SearchCommand::Search {
                query: query1,
                root: LocationRef::from_location(&Location::local("/root")),
                event_mode: SearchEventMode::Location,
                session: session1,
                request: crate::model::request::RequestId::new(),
            })
            .unwrap();
        cmd_tx.send(SearchCommand::Cancel(session1)).unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query: query2,
                root: LocationRef::from_location(&Location::local("/root")),
                event_mode: SearchEventMode::Location,
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
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session1 = SessionId::new();
        let session2 = SessionId::new();
        let location = Location::local("/root");

        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                event_mode: SearchEventMode::Location,
                session: session1,
                request: RequestId::new(),
            })
            .unwrap();
        cmd_tx.send(SearchCommand::Cancel(session1)).unwrap();
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                event_mode: SearchEventMode::Location,
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
                let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();

        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&Location::local("/root")),
                event_mode: SearchEventMode::Location,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&Location::local("/root")),
                event_mode: SearchEventMode::Location,
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
            "stale search request should not emit SearchResults"
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
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry.clone());

        let session = SessionId::new();
        let stale_request = RequestId::new();
        let fresh_request = RequestId::new();
        let location = Location::local("/root");

        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                event_mode: SearchEventMode::Location,
                session,
                request: stale_request,
            })
            .unwrap();
        tokio::task::yield_now().await;
        cmd_tx
            .send(SearchCommand::Search {
                query: SearchQuery::parse("target").unwrap(),
                root: LocationRef::from_location(&location),
                event_mode: SearchEventMode::Location,
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
