#[cfg(test)]
mod searcher_location_tests {
    use super::*;
    use crate::model::location::{Location, LocationDescriptor, LocationId, LocationRef};

    #[tokio::test]
    async fn test_search_location_emits_entry_results() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("found.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let request = RequestId::new();
        let location = Location::local("/root");
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("found").unwrap(),
                root: LocationRef::from_location(&location),
                session,
                request,
            })
            .unwrap();

        let matches = wait_for_search_entries_complete(&evt_rx, session).await;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "found.txt");
        assert_eq!(
            matches[0].location.descriptor(),
            Some(&LocationDescriptor::local("/root/found.txt"))
        );
    }

    #[tokio::test]
    async fn test_search_location_accepts_descriptor_only_ref() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/root",
            vec![MockProvider::make_file("found.txt", "/root", 100)],
        );

        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("found").unwrap(),
                root: LocationRef::descriptor_only(LocationDescriptor::local("/root")),
                session,
                request: RequestId::new(),
            })
            .unwrap();

        let matches = wait_for_search_entries_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "found.txt");
    }

    #[tokio::test]
    async fn test_search_location_id_only_without_registry_entry_emits_error() {
        let provider = MockProvider::new();
        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let request = RequestId::new();
        let missing_id = LocationId(999);
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("found").unwrap(),
                root: LocationRef::id_only(missing_id),
                session,
                request,
            })
            .unwrap();

        let event = wait_for_error(&evt_rx, session).await;
        match event {
            Event::Error {
                code,
                target,
                request: error_request,
                ..
            } => {
                assert_eq!(code, ErrorCode::LocationUnresolved);
                assert_eq!(target, Some(ErrorTarget::Location(missing_id)));
                assert_eq!(error_request, Some(request));
            }
            other => panic!("expected Error event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_search_location_segmented_route_emits_error() {
        let provider = MockProvider::new();
        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let request = RequestId::new();
        let descriptor = LocationDescriptor::local("/root.zip").archive_member("inside");
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("found").unwrap(),
                root: LocationRef::descriptor_only(descriptor),
                session,
                request,
            })
            .unwrap();

        let event = wait_for_error(&evt_rx, session).await;
        match event {
            Event::Error {
                code,
                request: error_request,
                ..
            } => {
                assert_eq!(code, ErrorCode::LocationSegmentedUnsupported);
                assert_eq!(error_request, Some(request));
            }
            other => panic!("expected Error event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_search_location_unsupported_provider_emits_error() {
        let provider = MockProvider::new();
        let registry = NodeRegistry::new();
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let request = RequestId::new();
        let descriptor = LocationDescriptor::provider_profile("sftp", "work", "/remote");
        cmd_tx
            .send(SearchCommand::SearchLocation {
                query: SearchQuery::parse("found").unwrap(),
                root: LocationRef::descriptor_only(descriptor),
                session,
                request,
            })
            .unwrap();

        let event = wait_for_error(&evt_rx, session).await;
        match event {
            Event::Error {
                code,
                request: error_request,
                ..
            } => {
                assert_eq!(code, ErrorCode::UnsupportedProvider);
                assert_eq!(error_request, Some(request));
            }
            other => panic!("expected Error event, got {other:?}"),
        }
    }
}
