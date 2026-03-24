use std::sync::Arc;

use flume::{Receiver, Sender};

use crate::actors::router::CommandRouter;
use crate::actors::ActorSystem;
use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::module::{HandlerContext, HandlerRegistry, Module, ModuleContext};
use crate::api::session_manager::SessionManager;
use crate::utils::channel::send_or_warn;
use crate::errors::CoreError;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;

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
/// let scan = ScanModule::new(Arc::new(LocalFs::new(core.registry())));
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
    /// Event sender — shared with actors via HandlerContext
    event_tx: Sender<Event>,
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

        // ── API boundary channels ────────────────────────────────────
        let (command_tx, command_rx) = flume::unbounded();
        let (event_tx, event_rx) = flume::unbounded();

        // ── Shared state ─────────────────────────────────────────────
        let registry = NodeRegistry::new();
        let sessions = SessionManager::new(registry.clone());
        let handlers = Arc::new(HandlerRegistry::new());

        // ── Context shared with all handlers ─────────────────────────
        let ctx = HandlerContext {
            events: event_tx.clone(),
            sessions: sessions.clone(),
            registry: registry.clone(),
        };

        // ── Register built-in session lifecycle handlers ─────────────
        handlers.on("session.handshake", |_cmd, ctx| {
            let session = ctx.sessions.create_session(ctx.events.clone());
            send_or_warn(&ctx.events, Event::SessionCreated(session), "emit SessionCreated");
        });

        handlers.on("session.destroy", |cmd, ctx| {
            if let Command::DestroySession(session_id) = cmd {
                ctx.sessions.remove(session_id);
                send_or_warn(&ctx.events, Event::SessionDestroyed(session_id), "emit SessionDestroyed");
            }
        });

        // ── Spawn the generic Router ─────────────────────────────────
        let router = CommandRouter::new(command_rx, handlers.clone(), ctx);
        system.spawn(router);

        Self {
            command_tx,
            event_rx,
            event_tx,
            actor_system: system,
            handlers,
            sessions,
            registry,
        }
    }

    /// Create a FilerCore with all built-in modules loaded.
    ///
    /// Equivalent to `FilerCore::new()` + loading NavigationModule,
    /// ScanModule, and all other available modules.
    pub fn with_defaults() -> Self {
        
        // Built-in modules will be loaded here as they are created:
        // core.load(ScanModule::new(...));
        // core.load(NavigationModule::new(...));
        // etc.
        Self::new()
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
            events: self.event_tx.clone(),
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
    /// Returns `Err(CoreError::ChannelClosed)` if the system has been
    /// shut down.
    pub fn send(&self, command: Command) -> Result<(), CoreError> {
        self.command_tx
            .send(command)
            .map_err(|_| CoreError::ChannelClosed("command channel closed".into()))
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
    pub fn event_sender(&self) -> Sender<Event> {
        self.event_tx.clone()
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

    /// Shut down all actors.
    ///
    /// Aborts every spawned task, which drops their channel endpoints
    /// and cascades shutdown through the system. After shutdown,
    /// [`send()`](Self::send) will return `Err(CoreError::ChannelClosed)`.
    pub fn shutdown(&self) -> Result<(), CoreError> {
        self.actor_system.shutdown();
        Ok(())
    }
}

impl Default for FilerCore {
    fn default() -> Self {
        Self::with_defaults()
    }
}