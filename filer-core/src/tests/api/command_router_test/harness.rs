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
    // Compatibility pin for API-006: the harness registry remains only for
    // routing commands that still accept NodeId.
    use crate::model::node::NodeId;
    use crate::model::operation::OperationId;
    use crate::model::registry::NodeRegistry;
    use crate::model::request::RequestId;
    use crate::model::session::SessionId;
    use crate::modules::navigation::navigator::NavCommand;
    use crate::modules::operations::operator::{OperationEventMode, OpsCommand};
    use crate::modules::preview::previewer::{PreviewCommand, PreviewEventMode};
    use crate::modules::scan::scanner::ScanCommand;
    use crate::modules::search::searcher::{SearchCommand, SearchEventMode};
    use crate::modules::watch::watcher::{UnwatchScope, WatchCommand, WatchEventMode};
    use crate::modules::compat;
    use crate::pipeline::PipelineConfig;
    use crate::utils::channel::send_or_warn;

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
        /// Compatibility-only registry used for NodeId resolution.
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

            register_test_handlers(
                &handlers,
                nav_tx.clone(),
                scan_tx.clone(),
                search_tx.clone(),
                watch_tx.clone(),
                preview_tx.clone(),
                ops_tx.clone(),
            );

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
