use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};

use crate::actors::ActorSystem;
use crate::actors::router::CommandRouter;
use crate::api::commands::Command;
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::api::module::{HandlerContext, HandlerRegistry, Module, ModuleContext};
use crate::api::session_manager::SessionManager;
use crate::errors::CoreError;
use crate::model::operation::OperationId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::navigation::NavigationModule;
use crate::modules::operations::OperationsModule;
use crate::modules::preview::PreviewModule;
use crate::modules::scan::ScanModule;
use crate::modules::search::SearchModule;
use crate::modules::watch::WatchModule;
use crate::services::dir_cache::DirCache;
use crate::utils::channel::send_or_warn;
use crate::vfs::local::LocalFs;
use crate::vfs::local_watch::LocalWatchProvider;

/// Public API handle for the filer core runtime.
///
/// FilerCore provides the bus infrastructure (events,
/// sessions, registry, actor lifecycle) and lets modules
/// supply the actual behavior.
///
/// # Setup
///
/// ```ignore
/// use filer_core::{FilerCore, LocalFs};
/// use filer_core::modules::{NavigationModule, ScanModule};
///
/// let core = FilerCore::new();
///
/// // Compose your system from modules
/// let scan = ScanModule::new(Arc::new(LocalFs::new()));
/// let nav = NavigationModule::new(scan.sender());
/// core.load(scan);
/// core.load(nav);
///
/// // Or use the convenience method
/// let core = FilerCore::with_defaults();
///
/// // Send commands, receive events
/// core.send(Command::Handshake)?;
/// let event = core.event_receiver().recv_async().await?;
/// ```
///
/// # Dynamic extensibility
///
/// - **Add a module**: `core.load(MyModule)` at any time
/// - **Swap an actor**: load your module for the same command keys
/// - **Custom commands**: use `Command::Extension { key, payload, session }`
/// - **Quick handler**: `core.on("key", handler)` for stateless logic
pub struct FilerCore {
    /// Command sender — UI writes here
    command_tx: Sender<Command>,
    /// Event receiver — UI reads here
    event_rx: Receiver<Event>,
    /// Bounded event sink shared by handlers and actors.
    event_sink: EventSink,
    /// Manages spawned actor tasks
    actor_system: ActorSystem,
    /// Command handler registry (shared with Router)
    handlers: Arc<HandlerRegistry>,
    /// Session manager (shared with Router via HandlerContext)
    sessions: SessionManager,
    /// Node registry (shared everywhere)
    registry: NodeRegistry,
}

impl FilerCore {
    /// Create a new FilerCore runtime.
    ///
    /// This creates the bus infrastructure and spawns the generic
    /// CommandRouter. No domain actors are started — use [`load()`](Self::load)
    /// to add modules, or [`with_defaults()`](Self::with_defaults) for a
    /// batteries-included setup.
    ///
    /// Session lifecycle handlers (Handshake / DestroySession) are
    /// registered automatically — they're infrastructure, not a module concern.
    pub fn new() -> Self {
        let system = ActorSystem::new();

        let (command_tx, command_rx) = flume::unbounded();
        let (event_sink, event_rx) = EventSink::for_runtime(system.work_tracker());

        let registry = NodeRegistry::new();
        let sessions = SessionManager::new(registry.clone());
        let handlers = Arc::new(HandlerRegistry::new());

        let ctx = HandlerContext {
            events: event_sink.clone(),
            sessions: sessions.clone(),
            registry: registry.clone(),
        };

        handlers.on("session.handshake", |_cmd, ctx| {
            let session = ctx.sessions.create_session(ctx.events.clone());
            send_or_warn(
                &ctx.events,
                Event::SessionCreated(session),
                "emit SessionCreated",
            );
        });

        handlers.on("session.destroy", |cmd, ctx| {
            if let Command::DestroySession(session_id) = cmd {
                ctx.sessions.remove(session_id);
                send_or_warn(
                    &ctx.events,
                    Event::SessionDestroyed(session_id),
                    "emit SessionDestroyed",
                );
            }
        });

        let router = CommandRouter::new(command_rx, handlers.clone(), ctx);
        system.spawn(router);

        Self {
            command_tx,
            event_rx,
            event_sink,
            actor_system: system,
            handlers,
            sessions,
            registry,
        }
    }

    /// Create a FilerCore with all built-in modules loaded.
    ///
    /// Equivalent to `FilerCore::new()` + loading all six built-in modules
    /// backed by the local filesystem and a shared 128 MB directory cache.
    pub fn with_defaults() -> Self {
        let core = Self::new();
        let provider = Arc::new(LocalFs::new());
        let cache = Arc::new(Mutex::new(DirCache::new(128 * 1024 * 1024)));

        let scan = ScanModule::with_cache(provider.clone(), cache.clone());
        let nav = NavigationModule::new(scan.sender());
        let nav_tx = nav.sender();
        core.load(scan);
        core.load(nav);
        core.load(WatchModule::with_refresh(
            Arc::new(LocalWatchProvider::new()),
            nav_tx,
        ));
        core.load(SearchModule::new(provider.clone()));
        core.load(PreviewModule::new(provider.clone()));
        core.load(OperationsModule::with_cache(provider, cache));
        core
    }

    /// Load a module into the running system.
    ///
    /// The module's `init()` is called immediately, which registers
    /// command handlers and spawns actors. After this returns, the
    /// module is consumed and its actors are running.
    ///
    /// Modules can be loaded at any time — even while the system is
    /// processing commands. New handlers take effect immediately.
    pub fn load<M: Module>(&self, module: M) {
        let ctx = ModuleContext {
            events: self.event_sink.clone(),
            sessions: &self.sessions,
            registry: &self.registry,
            actors: &self.actor_system,
            handlers: &self.handlers,
        };
        Box::new(module).init(ctx);
    }

    /// Register a single command handler without the Module ceremony.
    ///
    /// Useful for quick one-off handlers or testing.
    pub fn on(
        &self,
        key: impl Into<String>,
        handler: impl Fn(Command, &HandlerContext) + Send + Sync + 'static,
    ) {
        self.handlers.on(key, handler);
    }

    /// Register a cleanup hook called when a session is destroyed.
    pub fn on_session_destroy(
        &self,
        hook: impl Fn(SessionId, &HandlerContext) + Send + Sync + 'static,
    ) {
        self.handlers.on_session_destroy(hook);
    }

    /// Send a command into the core.
    ///
    /// Returns `Err(CoreError)` with `ErrorCode::ChannelClosed` if the system has been
    /// shut down.
    pub fn send(&self, command: Command) -> Result<(), CoreError> {
        self.command_tx
            .send(command)
            .map_err(|_| CoreError::channel_closed("command"))
    }

    /// Try to receive a pending event without blocking.
    pub fn try_recv(&self) -> Option<Event> {
        self.event_rx.try_recv().ok()
    }

    /// Clone of the event receiver — for async consumption.
    ///
    /// Flume receivers are multi-consumer: clones compete for messages
    /// (each message goes to exactly one consumer).
    pub fn event_receiver(&self) -> Receiver<Event> {
        self.event_rx.clone()
    }

    /// Clone of the command sender — for sending from multiple threads.
    pub fn command_sender(&self) -> Sender<Command> {
        self.command_tx.clone()
    }

    /// Clone of the event sender — custom actors can use this to
    /// emit events into the system.
    pub fn event_sender(&self) -> EventSink {
        self.event_sink.clone()
    }

    /// Access the shared node registry.
    ///
    /// Modules and external code use this for path ↔ NodeId resolution.
    pub fn registry(&self) -> NodeRegistry {
        self.registry.clone()
    }

    /// Access the session manager.
    pub fn sessions(&self) -> SessionManager {
        self.sessions.clone()
    }

    /// Generate a new request ID for correlating async command results.
    pub fn next_request_id(&self) -> RequestId {
        RequestId::new()
    }

    /// Generate a new operation ID for correlating file operation events.
    pub fn next_operation_id(&self) -> OperationId {
        OperationId::new()
    }

    /// Shut down the runtime and wait for all in-flight work to stop.
    ///
    /// Shutdown cancels tracked command work, aborts actor loops, joins every
    /// task, and only then returns. After it returns, [`send()`](Self::send)
    /// will return a channel-closed [`CoreError`].
    pub async fn shutdown(&self) -> Result<(), CoreError> {
        self.actor_system.shutdown().await
    }
}

impl Default for FilerCore {
    fn default() -> Self {
        Self::with_defaults()
    }
}
