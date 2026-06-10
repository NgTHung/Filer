//! Tests for the Command Router
//!
//! The Command Router is a generic dispatcher backed by HandlerRegistry.
//! It receives `Command` from the API layer and:
//! 1. Validates the session via SessionManager
//! 2. Looks up the handler by `Command::key()` in the HandlerRegistry
//! 3. Calls the handler closure (which forwards to the appropriate actor channel)
//! 4. For `DestroySession`, runs registered destroy hooks
//!
//! Session lifecycle (Handshake / DestroySession) is registered as normal
//! handlers — the Router has no special knowledge of them.
//!
//! Tests are written BEFORE implementation (TDD).

#[cfg(test)]
mod command_router_tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use flume::{Receiver, Sender};
    use tokio::time::timeout;

    use crate::actors::Actor;
    use crate::actors::router::CommandRouter;
    use crate::api::commands::Command;
    use crate::api::events::Event;
    use crate::api::module::{HandlerContext, HandlerRegistry};
    use crate::api::session_manager::SessionManager;
    use crate::errors::{CoreError, ErrorCode};
    use crate::model::location::{Location, LocationRef};
    use crate::model::node::NodeId;
    use crate::model::operation::OperationId;
    use crate::model::registry::NodeRegistry;
    use crate::model::request::RequestId;
    use crate::model::session::SessionId;
    use crate::modules::navigation::navigator::NavCommand;
    use crate::modules::operations::operator::OpsCommand;
    use crate::modules::preview::previewer::PreviewCommand;
    use crate::modules::scan::scanner::ScanCommand;
    use crate::modules::search::searcher::SearchCommand;
    use crate::modules::watch::watcher::WatchCommand;
    use crate::pipeline::PipelineConfig;

    /// Timeout for async operations in tests
    const TEST_TIMEOUT: Duration = Duration::from_millis(500);

    /// Helper to create a CommandRouter with handlers registered via HandlerRegistry.
    ///
    /// Mimics what modules do: registers handler closures that forward
    /// commands to per-actor channels. This lets us test the Router's
    /// session validation and dispatch logic in isolation.
    struct RouterTestHarness {
        /// Send commands into the router
        command_tx: Sender<Command>,
        /// Receive events from the router (errors, session events, etc.)
        event_rx: Receiver<Event>,
        /// Clone of event sender — used to create sessions in SessionManager
        event_tx: Sender<Event>,
        /// Receive NavCommands that the router dispatches
        nav_rx: Receiver<NavCommand>,
        /// Receive ScanCommands that the router dispatches
        scan_rx: Receiver<ScanCommand>,
        /// Receive SearchCommands that the router dispatches
        search_rx: Receiver<SearchCommand>,
        /// Receive WatchCommands that the router dispatches
        watch_rx: Receiver<WatchCommand>,
        /// Receive PreviewCommands that the router dispatches
        preview_rx: Receiver<PreviewCommand>,
        /// Receive OpsCommands that the router dispatches
        ops_rx: Receiver<OpsCommand>,
        /// The registry used for NodeId resolution
        registry: NodeRegistry,
        /// Session manager clone — shares state with the router
        session_manager: SessionManager,
    }

    impl RouterTestHarness {
        /// Create a new test harness with the router running in background.
        ///
        /// Registers handlers for all command keys (mimicking what the
        /// built-in modules do), plus session lifecycle handlers.
        fn new() -> Self {
            let (command_tx, command_rx) = flume::unbounded::<Command>();
            let (event_tx, event_rx) = flume::unbounded::<Event>();
            let (nav_tx, nav_rx) = flume::unbounded::<NavCommand>();
            let (scan_tx, scan_rx) = flume::unbounded::<ScanCommand>();
            let (search_tx, search_rx) = flume::unbounded::<SearchCommand>();
            let (watch_tx, watch_rx) = flume::unbounded::<WatchCommand>();
            let (preview_tx, preview_rx) = flume::unbounded::<PreviewCommand>();
            let (ops_tx, ops_rx) = flume::unbounded::<OpsCommand>();
            let registry = NodeRegistry::new();
            let session_manager = SessionManager::new(registry.clone());

            let handlers = Arc::new(HandlerRegistry::new());
            let ctx = HandlerContext {
                events: event_tx.clone(),
                sessions: session_manager.clone(),
                registry: registry.clone(),
            };

            handlers.on("session.handshake", |_cmd, ctx| {
                let session = ctx.sessions.create_session(ctx.events.clone());
                let _ = ctx.events.send(Event::SessionCreated(session));
            });
            handlers.on("session.destroy", |cmd, ctx| {
                if let Command::DestroySession(session_id) = cmd {
                    ctx.sessions.remove(session_id);
                    let _ = ctx.events.send(Event::SessionDestroyed(session_id));
                }
            });

            {
                let tx = nav_tx.clone();
                handlers.on("navigate.path.compat", move |cmd, _ctx| {
                    if let Command::NavigatePathCompat {
                        path,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(NavCommand::NavigateToPath {
                            session,
                            path,
                            request,
                        });
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate", move |cmd, _ctx| {
                    if let Command::Navigate {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(NavCommand::NavigateToLocation {
                            session,
                            location,
                            request,
                        });
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.node.compat", move |cmd, _ctx| {
                    if let Command::NavigateNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(NavCommand::Navigate {
                            session,
                            node,
                            request,
                        });
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.up", move |cmd, _ctx| {
                    if let Command::NavigateUp { session, request } = cmd {
                        let _ = tx.send(NavCommand::Up(session, request));
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.back", move |cmd, _ctx| {
                    if let Command::NavigateBack { session, request } = cmd {
                        let _ = tx.send(NavCommand::Back(session, request));
                    }
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on("navigate.refresh", move |cmd, _ctx| {
                    if let Command::Refresh { session, request } = cmd {
                        let _ = tx.send(NavCommand::Refresh(session, request));
                    }
                });
            }

            {
                let tx = search_tx.clone();
                handlers.on("search.node.compat", move |cmd, ctx| {
                    if let Command::SearchNodeCompat {
                        query,
                        root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                let _ = tx.send(SearchCommand::Search {
                                    query,
                                    root,
                                    session,
                                    request,
                                });
                            }
                            Err(error) => {
                                let _ = ctx.events.send(Event::from_request_error(
                                    CoreError::invalid_input(format!(
                                        "Invalid search query: {error}"
                                    )),
                                    session,
                                    request,
                                ));
                            }
                        }
                    }
                });
            }
            {
                let tx = search_tx.clone();
                handlers.on("search.path.compat", move |cmd, ctx| {
                    if let Command::SearchPathCompat {
                        query,
                        root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                let _ = tx.send(SearchCommand::SearchPath {
                                    query,
                                    root,
                                    session,
                                    request,
                                });
                            }
                            Err(error) => {
                                let _ = ctx.events.send(Event::from_request_error(
                                    CoreError::invalid_input(format!(
                                        "Invalid search query: {error}"
                                    )),
                                    session,
                                    request,
                                ));
                            }
                        }
                    }
                });
            }
            {
                let tx = search_tx.clone();
                handlers.on("search", move |cmd, ctx| {
                    if let Command::Search {
                        query,
                        root,
                        session,
                        request,
                    } = cmd
                    {
                        match crate::model::query::SearchQuery::parse(&query) {
                            Ok(query) => {
                                let _ = tx.send(SearchCommand::SearchLocation {
                                    query,
                                    root,
                                    session,
                                    request,
                                });
                            }
                            Err(error) => {
                                let _ = ctx.events.send(Event::from_request_error(
                                    CoreError::invalid_input(format!(
                                        "Invalid search query: {error}"
                                    )),
                                    session,
                                    request,
                                ));
                            }
                        }
                    }
                });
            }
            {
                let tx = search_tx.clone();
                handlers.on("search.cancel", move |cmd, _ctx| {
                    if let Command::CancelSearch { session } = cmd {
                        let _ = tx.send(SearchCommand::Cancel(session));
                    }
                });
            }

            {
                let tx = scan_tx.clone();
                handlers.on("scan.path.compat", move |cmd, _ctx| {
                    if let Command::ScanPathCompat {
                        path,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(ScanCommand::Scan {
                            path,
                            session,
                            pipeline,
                            load,
                            request,
                        });
                    }
                });
            }
            {
                let tx = scan_tx.clone();
                handlers.on("scan", move |cmd, _ctx| {
                    if let Command::Scan {
                        location,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(ScanCommand::ScanLocation {
                            location,
                            session,
                            pipeline,
                            load,
                            request,
                        });
                    }
                });
            }
            {
                let tx = scan_tx.clone();
                handlers.on("scan.node.compat", move |cmd, _ctx| {
                    if let Command::ScanNodeCompat {
                        node,
                        session,
                        pipeline,
                        load,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(ScanCommand::ScanNode {
                            node,
                            session,
                            pipeline,
                            load,
                            request,
                        });
                    }
                });
            }

            {
                let tx = watch_tx.clone();
                handlers.on("watch.node.compat", move |cmd, _ctx| {
                    if let Command::WatchNodeCompat { node, session } = cmd {
                        let _ = tx.send(WatchCommand::Watch(node, session));
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch", move |cmd, _ctx| {
                    if let Command::Watch {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(WatchCommand::WatchLocation {
                            location,
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.node.remove.compat", move |cmd, _ctx| {
                    if let Command::UnwatchNodeCompat { node } = cmd {
                        let _ = tx.send(WatchCommand::Unwatch(node));
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.remove", move |cmd, _ctx| {
                    if let Command::Unwatch { location, session } = cmd {
                        let _ = tx.send(WatchCommand::UnwatchLocation { location, session });
                    }
                });
            }
            {
                let tx = watch_tx.clone();
                handlers.on("watch.session_remove", move |cmd, _ctx| {
                    if let Command::UnwatchSession(session) = cmd {
                        let _ = tx.send(WatchCommand::UnwatchSession(session));
                    }
                });
            }

            {
                let tx = preview_tx.clone();
                handlers.on("preview.load.node.compat", move |cmd, _ctx| {
                    if let Command::LoadPreviewNodeCompat {
                        id,
                        options,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::Generate {
                            path: id,
                            options,
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("preview.load", move |cmd, _ctx| {
                    if let Command::LoadPreview {
                        location,
                        options,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::GenerateLocation {
                            location,
                            options,
                            session,
                            request,
                        });
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("preview.cancel", move |cmd, _ctx| {
                    if let Command::CancelPreview { session } = cmd {
                        let _ = tx.send(PreviewCommand::Cancel(session));
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.load.node.compat", move |cmd, _ctx| {
                    if let Command::LoadMetadataNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::LoadMetadata(node, session, request));
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.load", move |cmd, _ctx| {
                    if let Command::LoadMetadata {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::LoadMetadataLocation(
                            location, session, request,
                        ));
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.extended.node.compat", move |cmd, _ctx| {
                    if let Command::LoadExtendedMetadataNodeCompat {
                        node,
                        session,
                        request,
                    } = cmd
                    {
                        let _ =
                            tx.send(PreviewCommand::LoadExtendedMetadata(node, session, request));
                    }
                });
            }
            {
                let tx = preview_tx.clone();
                handlers.on("metadata.extended", move |cmd, _ctx| {
                    if let Command::LoadExtendedMetadata {
                        location,
                        session,
                        request,
                    } = cmd
                    {
                        let _ = tx.send(PreviewCommand::LoadExtendedMetadataLocation(
                            location, session, request,
                        ));
                    }
                });
            }

            {
                let tx = ops_tx.clone();
                handlers.on("ops.copy.node.compat", move |cmd, _ctx| {
                    if let Command::CopyNodeCompat {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Copy {
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.copy", move |cmd, _ctx| {
                    if let Command::Copy {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CopyLocation {
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.move.node.compat", move |cmd, _ctx| {
                    if let Command::MoveNodeCompat {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Move {
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.move", move |cmd, _ctx| {
                    if let Command::Move {
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::MoveLocation {
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.delete.node.compat", move |cmd, _ctx| {
                    if let Command::DeleteNodeCompat {
                        nodes,
                        trash,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Delete {
                            targets: nodes,
                            trash,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.delete", move |cmd, _ctx| {
                    if let Command::Delete {
                        locations,
                        trash,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::DeleteLocation {
                            targets: locations,
                            trash,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.rename.node.compat", move |cmd, _ctx| {
                    if let Command::RenameNodeCompat {
                        node,
                        new_name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::Rename {
                            source: node,
                            new_name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.rename", move |cmd, _ctx| {
                    if let Command::Rename {
                        location,
                        new_name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::RenameLocation {
                            source: location,
                            new_name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_folder.node.compat", move |cmd, _ctx| {
                    if let Command::CreateFolderNodeCompat {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CreateFolder {
                            parent,
                            name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_folder", move |cmd, _ctx| {
                    if let Command::CreateFolder {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CreateFolderLocation {
                            parent,
                            name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_file.node.compat", move |cmd, _ctx| {
                    if let Command::CreateFileNodeCompat {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CreateFile {
                            parent,
                            name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.create_file", move |cmd, _ctx| {
                    if let Command::CreateFile {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    } = cmd
                    {
                        let _ = tx.send(OpsCommand::CreateFileLocation {
                            parent,
                            name,
                            session,
                            request,
                            operation,
                        });
                    }
                });
            }
            {
                let tx = ops_tx.clone();
                handlers.on("ops.cancel", move |cmd, _ctx| {
                    if let Command::CancelOperation { session, operation } = cmd {
                        let _ = tx.send(OpsCommand::CancelOperation { session, operation });
                    }
                });
            }

            {
                let tx = watch_tx.clone();
                handlers.on_session_destroy(move |session, _ctx| {
                    let _ = tx.send(WatchCommand::UnwatchSession(session));
                });
            }
            {
                let tx = nav_tx.clone();
                handlers.on_session_destroy(move |session, _ctx| {
                    let _ = tx.send(NavCommand::RemoveSession(session));
                });
            }

            let router = CommandRouter::new(command_rx, handlers, ctx);
            tokio::spawn(async move { router.run().await });

            Self {
                command_tx,
                event_rx,
                event_tx,
                nav_rx,
                scan_rx,
                search_rx,
                watch_rx,
                preview_rx,
                ops_rx,
                registry,
                session_manager,
            }
        }

        /// Send a command and wait briefly for routing
        async fn send(&self, cmd: Command) {
            self.command_tx
                .send_async(cmd)
                .await
                .expect("Failed to send command");
            // Small yield to let the router process
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        /// Create a valid session directly in the SessionManager.
        /// Commands sent with this session ID will pass validation.
        fn create_valid_session(&self) -> SessionId {
            self.session_manager.create_session(self.event_tx.clone())
        }
    }

    #[tokio::test]
    async fn test_route_navigate_path_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/documents");

        let request = RequestId::new();
        harness
            .send(Command::NavigatePathCompat {
                path: path.clone(),
                session,
                request,
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::NavigateToPath {
                session: s,
                path: p,
                request: r,
            } => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(p, path, "Path must be forwarded correctly");
                assert_eq!(r, request, "RequestId must be forwarded correctly");
            }
            other => panic!("Expected NavCommand::NavigateToPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_navigate_location_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::Navigate {
                location: location_ref.clone(),
                session,
                request,
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::NavigateToLocation {
                session: s,
                location,
                request: r,
            } => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected NavCommand::NavigateToLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_navigate_to_node_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let node = NodeId(42);

        let request = RequestId::new();
        harness
            .send(Command::NavigateNodeCompat {
                node,
                session,
                request,
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::Navigate {
                session: s,
                node: n,
                request: r,
            } => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(n, node, "NodeId must be forwarded correctly");
                assert_eq!(r, request, "RequestId must be forwarded correctly");
            }
            other => panic!("Expected NavCommand::Navigate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_navigate_up_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        let request = RequestId::new();
        harness.send(Command::NavigateUp { session, request }).await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::Up(s, r) => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be preserved");
            }
            other => panic!("Expected NavCommand::Up, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_refresh_to_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        let request = RequestId::new();
        harness.send(Command::Refresh { session, request }).await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::Refresh(s, r) => {
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be preserved");
            }
            other => panic!("Expected NavCommand::Refresh, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_search_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // Register a path for the root NodeId so the router can resolve it
        let path = PathBuf::from("/home/user/projects");
        let registered_id = harness.registry.clone().register(path.clone());

        harness
            .send(Command::SearchNodeCompat {
                query: "*.rs".to_string(),
                root: registered_id,
                session,
                request: RequestId::new(),
            })
            .await;

        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand")
            .expect("SearchCommand channel closed");

        match search_cmd {
            SearchCommand::Search {
                query,
                root: r,
                session: s,
                ..
            } => {
                assert_eq!(query.text, "*.rs", "Query text must be forwarded");
                assert_eq!(r, registered_id, "Root NodeId must be forwarded");
                assert_eq!(s, session, "Session must be the same for both command");
            }
            other => panic!("Expected SearchCommand::Search, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_search_path_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();
        let root = PathBuf::from("/home/user/projects");

        harness
            .send(Command::SearchPathCompat {
                query: "*.rs".to_string(),
                root: root.clone(),
                session,
                request,
            })
            .await;

        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand")
            .expect("SearchCommand channel closed");

        match search_cmd {
            SearchCommand::SearchPath {
                query,
                root: r,
                session: s,
                request: req,
            } => {
                assert_eq!(query.text, "*.rs", "Query text must be forwarded");
                assert_eq!(r, root, "Root path must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(req, request, "RequestId must be forwarded");
            }
            other => panic!("Expected SearchCommand::SearchPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_search_location_to_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::Search {
                query: "report".to_string(),
                root: location_ref.clone(),
                session,
                request,
            })
            .await;

        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand")
            .expect("SearchCommand channel closed");

        match search_cmd {
            SearchCommand::SearchLocation {
                query,
                root,
                session: s,
                request: r,
            } => {
                assert_eq!(query.text, "report", "Query text must be forwarded");
                assert_eq!(root, location_ref, "LocationRef must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected SearchCommand::SearchLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_invalid_search_query_emits_request_error() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let request = RequestId::new();

        harness
            .send(Command::Search {
                query: "type:unknown".to_string(),
                root: LocationRef::from_location(&Location::local("/tmp/search")),
                session,
                request,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for invalid query error")
            .expect("Event channel closed");

        match event {
            Event::Error {
                code,
                session: s,
                request: error_request,
                ..
            } => {
                assert_eq!(code, ErrorCode::InputInvalid);
                assert_eq!(s, session);
                assert_eq!(error_request, Some(request));
            }
            other => panic!("Expected Event::Error, got {other:?}"),
        }

        assert!(
            harness.search_rx.try_recv().is_err(),
            "invalid query must not be forwarded to Searcher"
        );
    }

    #[tokio::test]
    async fn test_route_scan_location_to_scanner() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test");
        let location_ref = LocationRef::from_location(&location);
        let pipeline = PipelineConfig::default();
        let request = RequestId::new();

        harness
            .send(Command::Scan {
                location: location_ref.clone(),
                session,
                pipeline: pipeline.clone(),
                load: crate::DirectoryLoadOptions::unbounded(crate::ListingOptions::metadata()),
                request,
            })
            .await;

        let scan_cmd = timeout(TEST_TIMEOUT, harness.scan_rx.recv_async())
            .await
            .expect("Timed out waiting for ScanCommand")
            .expect("ScanCommand channel closed");

        match scan_cmd {
            ScanCommand::ScanLocation {
                location,
                session: s,
                pipeline: p,
                load,
                request: r,
            } => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(p, pipeline, "PipelineConfig must be forwarded");
                assert_eq!(
                    load,
                    crate::DirectoryLoadOptions::unbounded(crate::ListingOptions::metadata()),
                    "DirectoryLoadOptions must be forwarded"
                );
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected ScanCommand::ScanLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_compat_scans_to_scanner() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/tmp/compat-scan");
        let node = harness.registry.clone().register(path.clone());
        let pipeline = PipelineConfig::with_default_sort();
        let load = crate::DirectoryLoadOptions::bounded(32);
        let path_request = RequestId::new();
        let node_request = RequestId::new();

        harness
            .send(Command::ScanPathCompat {
                path: path.clone(),
                session,
                pipeline: pipeline.clone(),
                load: load.clone(),
                request: path_request,
            })
            .await;

        match timeout(TEST_TIMEOUT, harness.scan_rx.recv_async())
            .await
            .expect("Timed out waiting for path ScanCommand")
            .expect("ScanCommand channel closed")
        {
            ScanCommand::Scan {
                path: routed_path,
                session: routed_session,
                pipeline: routed_pipeline,
                load: routed_load,
                request,
            } => {
                assert_eq!(routed_path, path);
                assert_eq!(routed_session, session);
                assert_eq!(routed_pipeline, pipeline);
                assert_eq!(routed_load, load);
                assert_eq!(request, path_request);
            }
            other => panic!("Expected ScanCommand::Scan, got {other:?}"),
        }

        harness
            .send(Command::ScanNodeCompat {
                node,
                session,
                pipeline: pipeline.clone(),
                load: load.clone(),
                request: node_request,
            })
            .await;

        match timeout(TEST_TIMEOUT, harness.scan_rx.recv_async())
            .await
            .expect("Timed out waiting for node ScanCommand")
            .expect("ScanCommand channel closed")
        {
            ScanCommand::ScanNode {
                node: routed_node,
                session: routed_session,
                pipeline: routed_pipeline,
                load: routed_load,
                request,
            } => {
                assert_eq!(routed_node, node);
                assert_eq!(routed_session, session);
                assert_eq!(routed_pipeline, pipeline);
                assert_eq!(routed_load, load);
                assert_eq!(request, node_request);
            }
            other => panic!("Expected ScanCommand::ScanNode, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_route_cancel_to_searcher_when_searching() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // First start a search so the router knows this session is searching
        let root = harness.registry.clone().register(PathBuf::from("/tmp"));
        harness
            .send(Command::SearchNodeCompat {
                query: "test".to_string(),
                root,
                session,
                request: RequestId::new(),
            })
            .await;

        // Drain the search command
        let _ = timeout(TEST_TIMEOUT, harness.search_rx.recv_async()).await;

        // Now cancel
        harness.send(Command::CancelSearch { session }).await;

        let cancel_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for SearchCommand::Cancel")
            .expect("SearchCommand channel closed");

        match cancel_cmd {
            SearchCommand::Cancel(s) => {
                assert_eq!(s, session, "Session must be the same for Cancel request");
            }
            other => panic!("Expected SearchCommand::Cancel, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_watch_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/watched");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::WatchNodeCompat { node, session })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::Watch(n, s) => {
                assert_eq!(n, node, "Watch NodeId must be forwarded");
                assert_eq!(s, session, "Watch SessionId must be forwarded");
            }
            other => panic!("Expected WatchCommand::Watch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_unwatch_to_watcher() {
        let harness = RouterTestHarness::new();
        let path = PathBuf::from("/home/user/unwatched");
        let node = harness.registry.clone().register(path.clone());

        // Unwatch carries only NodeId, no SessionId
        harness.send(Command::UnwatchNodeCompat { node }).await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::Unwatch(n) => {
                assert_eq!(n, node, "Unwatch NodeId must be forwarded");
            }
            other => panic!("Expected WatchCommand::Unwatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_watch_location_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = LocationRef::from_location(&Location::local("/home/user/watched"));
        let request = RequestId::new();

        harness
            .send(Command::Watch {
                location: location.clone(),
                session,
                request,
            })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::WatchLocation {
                location: routed,
                session: s,
                request: r,
            } => {
                assert_eq!(routed, location);
                assert_eq!(s, session);
                assert_eq!(r, request);
            }
            other => panic!("Expected WatchCommand::WatchLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_unwatch_location_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = LocationRef::from_location(&Location::local("/home/user/watched"));

        harness
            .send(Command::Unwatch {
                location: location.clone(),
                session,
            })
            .await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::UnwatchLocation {
                location: routed,
                session: s,
            } => {
                assert_eq!(routed, location);
                assert_eq!(s, session);
            }
            other => panic!("Expected WatchCommand::UnwatchLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_unwatch_session_to_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::UnwatchSession(session)).await;

        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::UnwatchSession(s) => {
                assert_eq!(s, session, "UnwatchSession must forward SessionId");
            }
            other => panic!("Expected WatchCommand::UnwatchSession, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_preview_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/photo.jpg");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: node,
                options: None,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::Generate {
                path: p,
                options,
                session: s,
                ..
            } => {
                assert_eq!(p, node, "Preview NodeId must match request command");
                assert!(
                    options.is_none(),
                    "Options should be None when not provided"
                );
                assert_eq!(s, session, "Preview session id must match request command");
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_preview_location_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test/preview.txt");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::LoadPreview {
                location: location_ref.clone(),
                options: None,
                session,
                request,
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::GenerateLocation {
                location,
                options,
                session: s,
                request: r,
            } => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert!(options.is_none(), "Options must be forwarded as None");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!("Expected PreviewCommand::GenerateLocation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_cancel_preview_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::CancelPreview { session }).await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::Cancel(s) => {
                assert_eq!(s, session, "Cancel preview session must match the command");
            }
            other => panic!("Expected PreviewCommand::Cancel, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_metadata_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/document.pdf");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadMetadataNodeCompat {
                node,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadMetadata(p, s, _) => {
                assert_eq!(p, node, "Metadata NodeId must match request");
                assert_eq!(s, session, "Load request session id must match the command");
            }
            other => panic!("Expected PreviewCommand::LoadMetadata, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_load_metadata_location_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test/document.pdf");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::LoadMetadata {
                location: location_ref.clone(),
                session,
                request,
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadMetadataLocation(location, s, r) => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!(
                "Expected PreviewCommand::LoadMetadataLocation, got {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_route_load_extended_metadata_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/music.mp3");
        let node = harness.registry.clone().register(path.clone());

        harness
            .send(Command::LoadExtendedMetadataNodeCompat {
                node,
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadExtendedMetadata(p, s, _) => {
                assert_eq!(p, node, "Extended metadata NodeId must match request");
                assert_eq!(s, session, "Extended metadata session must match request");
            }
            other => panic!(
                "Expected PreviewCommand::LoadExtendedMetadata, got {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_route_load_extended_metadata_location_to_previewer() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let location = Location::local("/tmp/location-api-test/music.mp3");
        let location_ref = LocationRef::from_location(&location);
        let request = RequestId::new();

        harness
            .send(Command::LoadExtendedMetadata {
                location: location_ref.clone(),
                session,
                request,
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out waiting for PreviewCommand")
            .expect("PreviewCommand channel closed");

        match preview_cmd {
            PreviewCommand::LoadExtendedMetadataLocation(location, s, r) => {
                assert_eq!(location, location_ref, "LocationRef must be forwarded");
                assert_eq!(s, session, "SessionId must be preserved");
                assert_eq!(r, request, "RequestId must be forwarded");
            }
            other => panic!(
                "Expected PreviewCommand::LoadExtendedMetadataLocation, got {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_route_copy_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let src = harness
            .registry
            .clone()
            .register(PathBuf::from("/a/file.txt"));
        let dst = harness.registry.clone().register(PathBuf::from("/b"));
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CopyNodeCompat {
                sources: vec![src],
                destination: dst,
                session,
                request,
                operation,
            })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::Copy {
                sources,
                destination,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(sources, vec![src]);
                assert_eq!(destination, dst);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::Copy, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_create_file_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let parent = harness
            .registry
            .clone()
            .register(PathBuf::from("/home/user"));
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CreateFileNodeCompat {
                parent,
                name: "notes.txt".to_string(),
                session,
                request,
                operation,
            })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::CreateFile {
                parent: p,
                name,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(p, parent);
                assert_eq!(name, "notes.txt");
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::CreateFile, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_cancel_operation_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let operation = OperationId::new();

        harness
            .send(Command::CancelOperation { session, operation })
            .await;

        let ops_cmd = timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed");

        match ops_cmd {
            OpsCommand::CancelOperation {
                session: s,
                operation: op,
            } => {
                assert_eq!(s, session);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::CancelOperation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_location_write_commands_to_operator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let src = LocationRef::from_location(&Location::local("/a/file.txt"));
        let dst = LocationRef::from_location(&Location::local("/b"));
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::Copy {
                sources: vec![src.clone()],
                destination: dst.clone(),
                session,
                request,
                operation,
            })
            .await;

        match timeout(TEST_TIMEOUT, harness.ops_rx.recv_async())
            .await
            .expect("Timed out waiting for OpsCommand")
            .expect("OpsCommand channel closed")
        {
            OpsCommand::CopyLocation {
                sources,
                destination,
                session: s,
                request: r,
                operation: op,
            } => {
                assert_eq!(sources, vec![src]);
                assert_eq!(destination, dst);
                assert_eq!(s, session);
                assert_eq!(r, request);
                assert_eq!(op, operation);
            }
            other => panic!("Expected OpsCommand::CopyLocation, got {:?}", other),
        }
    }

    #[test]
    fn test_location_command_metadata() {
        let session = SessionId::new();
        let request = RequestId::new();
        let operation = OperationId::new();
        let location = LocationRef::from_location(&Location::local("/tmp/file.txt"));

        let copy = Command::Copy {
            sources: vec![location.clone()],
            destination: location.clone(),
            session,
            request,
            operation,
        };
        assert_eq!(copy.key(), "ops.copy");
        assert_eq!(copy.session_id(), Some(session));
        assert_eq!(copy.request_id(), Some(request));
        assert_eq!(copy.operation_id(), Some(operation));

        let watch = Command::Watch {
            location,
            session,
            request,
        };
        assert_eq!(watch.key(), "watch");
        assert_eq!(watch.session_id(), Some(session));
        assert_eq!(watch.request_id(), Some(request));
        assert_eq!(watch.operation_id(), None);

        let cancel_operation = Command::CancelOperation { session, operation };
        assert_eq!(cancel_operation.key(), "ops.cancel");
        assert_eq!(cancel_operation.session_id(), Some(session));
        assert_eq!(cancel_operation.request_id(), None);
        assert_eq!(cancel_operation.operation_id(), Some(operation));
    }

    #[tokio::test]
    async fn test_route_preserves_session_id_navigate() {
        let harness = RouterTestHarness::new();
        let session_a = harness.create_valid_session();
        let session_b = harness.create_valid_session();

        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/a"),
                session: session_a,
                request: RequestId::new(),
            })
            .await;
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/b"),
                session: session_b,
                request: RequestId::new(),
            })
            .await;

        // First command should carry session_a
        let cmd1 = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");
        match cmd1 {
            NavCommand::NavigateToPath { session, path, .. } => {
                assert_eq!(session, session_a);
                assert_eq!(path, PathBuf::from("/a"));
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }

        // Second command should carry session_b
        let cmd2 = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");
        match cmd2 {
            NavCommand::NavigateToPath { session, path, .. } => {
                assert_eq!(session, session_b);
                assert_eq!(path, PathBuf::from("/b"));
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_different_sessions_to_different_actors() {
        let harness = RouterTestHarness::new();
        let session_a = harness.create_valid_session();
        let session_b = harness.create_valid_session();

        // Session A navigates
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/home/a"),
                session: session_a,
                request: RequestId::new(),
            })
            .await;

        // Session B searches
        let root = harness
            .registry
            .clone()
            .register(PathBuf::from("/search/root"));
        harness
            .send(Command::SearchNodeCompat {
                query: "find me".to_string(),
                root,
                session: session_b,
                request: RequestId::new(),
            })
            .await;

        // Navigate should go to Navigator
        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for nav")
            .expect("Nav channel closed");
        match nav_cmd {
            NavCommand::NavigateToPath { session, .. } => {
                assert_eq!(session, session_a, "Navigate must carry session A");
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }

        // Search should go to Searcher
        let search_cmd = timeout(TEST_TIMEOUT, harness.search_rx.recv_async())
            .await
            .expect("Timed out waiting for search")
            .expect("Search channel closed");
        match search_cmd {
            SearchCommand::Search { .. } => {
                // Correctly routed to searcher
            }
            other => panic!("Expected SearchCommand::Search, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_multiple_sessions_interleaved() {
        let harness = RouterTestHarness::new();
        let session_1 = harness.create_valid_session();
        let session_2 = harness.create_valid_session();
        let session_3 = harness.create_valid_session();

        // Interleave commands from 3 sessions going to the same actor
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/s1"),
                session: session_1,
                request: RequestId::new(),
            })
            .await;
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/s2"),
                session: session_2,
                request: RequestId::new(),
            })
            .await;
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/s3"),
                session: session_3,
                request: RequestId::new(),
            })
            .await;

        // All should arrive at Navigator, in order, with correct session IDs
        let mut received_sessions = Vec::new();
        for _ in 0..3 {
            let cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
                .await
                .expect("Timed out")
                .expect("Channel closed");
            match cmd {
                NavCommand::NavigateToPath { session, .. } => {
                    received_sessions.push(session);
                }
                other => panic!("Expected NavigateToPath, got {:?}", other),
            }
        }

        assert_eq!(received_sessions.len(), 3);
        assert_eq!(received_sessions[0], session_1);
        assert_eq!(received_sessions[1], session_2);
        assert_eq!(received_sessions[2], session_3);
    }

    #[tokio::test]
    async fn test_route_handshake_emits_session_created() {
        let harness = RouterTestHarness::new();

        harness.send(Command::Handshake).await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for SessionCreated event")
            .expect("Event channel closed");

        match event {
            Event::SessionCreated(s) => {
                // A new session ID should have been generated
                assert_ne!(
                    s,
                    SessionId::DEFAULT,
                    "Generated session should not be DEFAULT"
                );
                // Session should now exist in the manager
                assert!(
                    harness.session_manager.exists(s),
                    "Session must exist after Handshake"
                );
            }
            other => panic!("Expected SessionCreated, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_destroy_session_cleans_up_navigator() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::DestroySession(session)).await;

        // DestroySession should send both UnwatchSession to watcher
        // and RemoveSession to navigator
        let _watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand::RemoveSession")
            .expect("NavCommand channel closed");

        match nav_cmd {
            NavCommand::RemoveSession(s) => {
                assert_eq!(s, session);
            }
            other => panic!("Expected NavCommand::RemoveSession, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_route_destroy_session_emits_event() {
        let harness = RouterTestHarness::new();
        // Must create a valid session first
        let session = harness.create_valid_session();

        harness.send(Command::DestroySession(session)).await;

        // DestroySession should send UnwatchSession to watcher
        let _watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand")
            .expect("WatchCommand channel closed");

        // And RemoveSession to navigator
        let _nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for NavCommand::RemoveSession")
            .expect("NavCommand channel closed");

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for SessionDestroyed event")
            .expect("Event channel closed");

        match event {
            Event::SessionDestroyed(s) => {
                assert_eq!(
                    s, session,
                    "SessionDestroyed must carry the correct session"
                );
                // Session should be removed from manager
                assert!(
                    !harness.session_manager.exists(session),
                    "Session must not exist after DestroySession"
                );
            }
            other => panic!("Expected SessionDestroyed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_destroy_session_unwatches_all_for_session() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness.send(Command::DestroySession(session)).await;

        // The router should send UnwatchSession to watcher to clean up
        let watch_cmd = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async())
            .await
            .expect("Timed out waiting for WatchCommand::UnwatchSession")
            .expect("WatchCommand channel closed");

        match watch_cmd {
            WatchCommand::UnwatchSession(s) => {
                assert_eq!(
                    s, session,
                    "UnwatchSession must carry the destroyed session"
                );
            }
            other => panic!("Expected WatchCommand::UnwatchSession, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_unknown_session_navigate_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new(); // Not registered in SessionManager

        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/home"),
                session: unknown,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                message,
                recoverable,
                session,
                ..
            } => {
                assert_eq!(session, unknown, "Error must carry the unknown session");
                assert!(recoverable, "Unknown session should be a recoverable error");
                assert!(
                    message.contains("Unknown session"),
                    "Error message should mention unknown session, got: {}",
                    message
                );
            }
            other => panic!("Expected Event::Error, got {:?}", other),
        }

        // Navigator should NOT have received anything
        let nav_result = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(
            nav_result.is_err(),
            "Navigator should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_location_command_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();
        let location = Location::local("/tmp/location-api-test");
        let request = RequestId::new();

        harness
            .send(Command::Navigate {
                location: LocationRef::from_location(&location),
                session: unknown,
                request,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                session,
                request: error_request,
                recoverable,
                message,
                ..
            } => {
                assert_eq!(session, unknown, "Error must carry the unknown session");
                assert_eq!(
                    error_request,
                    Some(request),
                    "Error must preserve the request id"
                );
                assert!(recoverable, "Unknown session should be recoverable");
                assert!(
                    message.contains("Unknown session"),
                    "Error message should mention unknown session, got: {}",
                    message
                );
            }
            other => panic!("Expected Event::Error, got {:?}", other),
        }

        let nav_result = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(
            nav_result.is_err(),
            "Navigator should not receive Location command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_search_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();
        let root = harness.registry.clone().register(PathBuf::from("/tmp"));

        harness
            .send(Command::SearchNodeCompat {
                query: "*.txt".to_string(),
                root,
                session: unknown,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error { session, .. } => {
                assert_eq!(session, unknown, "Error must carry the unknown session");
            }
            other => panic!("Expected Event::Error for unknown session, got {:?}", other),
        }

        // Searcher should NOT have received anything
        let search_result =
            timeout(Duration::from_millis(50), harness.search_rx.recv_async()).await;
        assert!(
            search_result.is_err(),
            "Searcher should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_watch_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();
        let node = harness.registry.clone().register(PathBuf::from("/watched"));

        harness
            .send(Command::WatchNodeCompat {
                node,
                session: unknown,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                session,
                recoverable,
                message,
                ..
            } => {
                assert_eq!(session, unknown);
                assert!(recoverable);
                assert!(message.contains("Unknown session"));
            }
            other => panic!("Expected Event::Error, got {:?}", other),
        }

        // Watcher should NOT have received anything
        let watch_result = timeout(Duration::from_millis(50), harness.watch_rx.recv_async()).await;
        assert!(
            watch_result.is_err(),
            "Watcher should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_unknown_session_ops_emits_error() {
        let harness = RouterTestHarness::new();
        let unknown = SessionId::new();
        let parent = harness.registry.clone().register(PathBuf::from("/home"));
        let request = RequestId::new();
        let operation = OperationId::new();

        harness
            .send(Command::CreateFolderNodeCompat {
                parent,
                name: "new_dir".to_string(),
                session: unknown,
                request,
                operation,
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out waiting for error event")
            .expect("Event channel closed");

        match event {
            Event::Error {
                session,
                request: err_request,
                operation: err_operation,
                ..
            } => {
                assert_eq!(session, unknown);
                assert_eq!(err_request, Some(request));
                assert_eq!(err_operation, Some(operation));
            }
            other => panic!("Expected Event::Error for unknown session, got {:?}", other),
        }

        // Operator should NOT have received anything
        let ops_result = timeout(Duration::from_millis(50), harness.ops_rx.recv_async()).await;
        assert!(
            ops_result.is_err(),
            "Operator should not receive command for unknown session"
        );
    }

    #[tokio::test]
    async fn test_commands_work_after_handshake() {
        let harness = RouterTestHarness::new();

        // Do a proper handshake to get a valid session
        harness.send(Command::Handshake).await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        let session = match event {
            Event::SessionCreated(s) => s,
            other => panic!("Expected SessionCreated, got {:?}", other),
        };

        // Now use that session to navigate
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/home"),
                session,
                request: RequestId::new(),
            })
            .await;

        let nav_cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
            .await
            .expect("Timed out waiting for nav command")
            .expect("Nav channel closed");

        match nav_cmd {
            NavCommand::NavigateToPath {
                session: s, path, ..
            } => {
                assert_eq!(s, session);
                assert_eq!(path, PathBuf::from("/home"));
            }
            other => panic!("Expected NavigateToPath, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_commands_fail_after_destroy() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // Verify it works before destroy
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/before"),
                session,
                request: RequestId::new(),
            })
            .await;
        let _ = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async()).await;

        // Destroy the session
        harness.send(Command::DestroySession(session)).await;
        // Drain destroy hooks: UnwatchSession, RemoveSession, and SessionDestroyed
        let _ = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async()).await;
        let _ = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async()).await;
        let _ = timeout(TEST_TIMEOUT, harness.event_rx.recv_async()).await;

        // Now try to use the destroyed session
        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/after"),
                session,
                request: RequestId::new(),
            })
            .await;

        let event = timeout(TEST_TIMEOUT, harness.event_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match event {
            Event::Error { session: s, .. } => {
                assert_eq!(s, session, "Error must reference the destroyed session");
            }
            other => panic!("Expected Event::Error after destroy, got {:?}", other),
        }

        // Navigator should NOT receive the post-destroy command
        let nav_result = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(
            nav_result.is_err(),
            "Navigator should not receive commands for destroyed session"
        );
    }

    #[tokio::test]
    async fn test_router_shuts_down_when_command_channel_closes() {
        let (command_tx, command_rx) = flume::unbounded::<Command>();
        let (event_tx, _event_rx) = flume::unbounded::<Event>();
        let registry = NodeRegistry::new();
        let session_manager = SessionManager::new(registry.clone());

        let handlers = Arc::new(HandlerRegistry::new());
        let ctx = HandlerContext {
            events: event_tx,
            sessions: session_manager,
            registry,
        };

        let router = CommandRouter::new(command_rx, handlers, ctx);
        let handle = tokio::spawn(async move { router.run().await });

        // Drop sender to close the command channel
        drop(command_tx);

        // Router should exit gracefully
        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(
            result.is_ok(),
            "Router should exit when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_router_processes_commands_sequentially() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        // Send multiple navigation commands rapidly
        for i in 0..5 {
            harness
                .send(Command::NavigatePathCompat {
                    path: PathBuf::from(format!("/dir/{}", i)),
                    session,
                    request: RequestId::new(),
                })
                .await;
        }

        // All 5 should arrive in order
        for i in 0..5 {
            let cmd = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async())
                .await
                .expect("Timed out")
                .expect("Channel closed");
            match cmd {
                NavCommand::NavigateToPath {
                    path, session: s, ..
                } => {
                    assert_eq!(s, session);
                    assert_eq!(path, PathBuf::from(format!("/dir/{}", i)));
                }
                other => panic!("Expected NavigateToPath, got {:?}", other),
            }
        }
    }

    #[tokio::test]
    async fn test_navigate_does_not_reach_searcher_or_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();

        harness
            .send(Command::NavigatePathCompat {
                path: PathBuf::from("/test"),
                session,
                request: RequestId::new(),
            })
            .await;

        // Navigator gets the command
        let nav = timeout(TEST_TIMEOUT, harness.nav_rx.recv_async()).await;
        assert!(nav.is_ok(), "Navigator should receive the command");

        // Searcher should NOT get anything
        let search = timeout(Duration::from_millis(50), harness.search_rx.recv_async()).await;
        assert!(
            search.is_err(),
            "Searcher should not receive Navigate commands"
        );

        // Watcher should NOT get anything
        let watch = timeout(Duration::from_millis(50), harness.watch_rx.recv_async()).await;
        assert!(
            watch.is_err(),
            "Watcher should not receive Navigate commands"
        );

        // Previewer should NOT get anything
        let preview = timeout(Duration::from_millis(50), harness.preview_rx.recv_async()).await;
        assert!(
            preview.is_err(),
            "Previewer should not receive Navigate commands"
        );
    }

    #[tokio::test]
    async fn test_search_does_not_reach_navigator_or_watcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let root = harness.registry.clone().register(PathBuf::from("/root"));

        harness
            .send(Command::SearchNodeCompat {
                query: "test".to_string(),
                root,
                session,
                request: RequestId::new(),
            })
            .await;

        // Searcher gets the command
        let search = timeout(TEST_TIMEOUT, harness.search_rx.recv_async()).await;
        assert!(search.is_ok(), "Searcher should receive the command");

        // Navigator should NOT get anything
        let nav = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(nav.is_err(), "Navigator should not receive Search commands");

        // Watcher should NOT get anything
        let watch = timeout(Duration::from_millis(50), harness.watch_rx.recv_async()).await;
        assert!(watch.is_err(), "Watcher should not receive Search commands");
    }

    #[tokio::test]
    async fn test_watch_does_not_reach_navigator_or_searcher() {
        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let node = harness.registry.clone().register(PathBuf::from("/watched"));

        harness
            .send(Command::WatchNodeCompat { node, session })
            .await;

        // Watcher gets the command
        let watch = timeout(TEST_TIMEOUT, harness.watch_rx.recv_async()).await;
        assert!(watch.is_ok(), "Watcher should receive the command");

        // Navigator should NOT get anything
        let nav = timeout(Duration::from_millis(50), harness.nav_rx.recv_async()).await;
        assert!(nav.is_err(), "Navigator should not receive Watch commands");

        // Searcher should NOT get anything
        let search = timeout(Duration::from_millis(50), harness.search_rx.recv_async()).await;
        assert!(
            search.is_err(),
            "Searcher should not receive Watch commands"
        );
    }

    #[tokio::test]
    async fn test_route_load_preview_with_options() {
        use crate::PreviewOptions;

        let harness = RouterTestHarness::new();
        let session = harness.create_valid_session();
        let path = PathBuf::from("/home/user/image.png");
        let node = harness.registry.clone().register(path.clone());

        let options = PreviewOptions {
            max_width: 800,
            max_height: 600,
            ..PreviewOptions::default()
        };

        harness
            .send(Command::LoadPreviewNodeCompat {
                id: node,
                options: Some(options.clone()),
                session,
                request: RequestId::new(),
            })
            .await;

        let preview_cmd = timeout(TEST_TIMEOUT, harness.preview_rx.recv_async())
            .await
            .expect("Timed out")
            .expect("Channel closed");

        match preview_cmd {
            PreviewCommand::Generate {
                path: p,
                options: o,
                session: s,
                ..
            } => {
                assert_eq!(
                    p, node,
                    "Preview request NodeId must match requested command"
                );
                assert_eq!(
                    s, session,
                    "Preview request session id must match requested command"
                );
                assert!(o.is_some(), "Options should be forwarded");
                let opts = o.unwrap();
                assert_eq!(opts.max_width, 800);
                assert_eq!(opts.max_height, 600);
            }
            other => panic!("Expected PreviewCommand::Generate, got {:?}", other),
        }
    }
}
