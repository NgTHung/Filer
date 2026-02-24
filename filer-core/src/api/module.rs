//! Module system — composable command handlers and actor wiring.
//!
//! A [`Module`] bundles command handlers with the actors they dispatch to.
//! Modules are the unit of composition for FilerCore: each module creates
//! its own internal channels, registers command handlers on the shared
//! [`HandlerRegistry`], and spawns its actors via the [`ActorSystem`].
//!
//! # How it works
//!
//! ```text
//! UI → Command → Router → HandlerRegistry::dispatch(key) → handler closure → actor channel
//! ```
//!
//! 1. Each `Command` variant maps to a string key via [`Command::key()`].
//! 2. Modules register handler closures by key during [`Module::init()`].
//! 3. The [`CommandRouter`] looks up the key and calls the handler.
//! 4. The handler captures the actor's `Sender<ActorCommand>` and forwards.
//!
//! # Dynamic extensibility
//!
//! - **Swap an actor**: don't load the built-in module, load your own that
//!   registers the same keys with a different actor behind them.
//! - **Add a capability**: create a new module with new keys and actors.
//! - **Extension commands**: use [`Command::Extension`] with a custom key
//!   and `Arc<dyn Any>` payload for commands not in the core enum.

use flume::Sender;

use crate::actors::ActorSystem;
use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::session_manager::SessionManager;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;

/// Context available to command handlers during dispatch.
///
/// Passed by reference to every handler invocation. Provides access
/// to the event bus, session manager, and node registry.
#[derive(Clone)]
pub struct HandlerContext {
    pub events: Sender<Event>,
    pub sessions: SessionManager,
    pub registry: NodeRegistry,
}

/// A command handler function.
///
/// Receives the full `Command` enum (the handler pattern-matches the variant
/// it cares about) and a `HandlerContext` for emitting events or querying
/// sessions/registry.
type HandlerFn = Box<dyn Fn(Command, &HandlerContext) + Send + Sync>;

/// A cleanup hook called when a session is destroyed.
///
/// Modules register these to clean up per-session state in their actors
/// (e.g., Navigator removes its session entry, Watcher stops watches).
type DestroyHookFn = Box<dyn Fn(SessionId, &HandlerContext) + Send + Sync>;

/// Registry mapping command keys to handler functions.
///
/// Shared (via `Arc`) between FilerCore (for registration) and
/// the CommandRouter (for dispatch). Thread-safe via `scc::HashMap`.
///
/// # Handler resolution
///
/// When the Router receives a `Command`, it calls `Command::key()` to get
/// the string key, then looks up and invokes the registered handler.
/// If no handler is found, the command is logged and dropped.
pub struct HandlerRegistry {
    handlers: scc::HashMap<String, HandlerFn>,
    destroy_hooks: std::sync::Mutex<Vec<DestroyHookFn>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: scc::HashMap::new(),
            destroy_hooks: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Register a handler for a command key.
    ///
    /// If a handler was already registered for this key, it is replaced.
    /// This enables modules to override built-in handlers.
    pub fn on(
        &self,
        key: impl Into<String>,
        handler: impl Fn(Command, &HandlerContext) + Send + Sync + 'static,
    ) {
        let key = key.into();
        // Remove existing (if any), then insert
        let _ = self.handlers.remove_sync(&key);
        let _ = self.handlers.insert_sync(key, Box::new(handler));
    }

    /// Register a cleanup hook called when a session is destroyed.
    ///
    /// Multiple hooks can be registered (one per module). They run
    /// after the session is removed from SessionManager.
    pub fn on_session_destroy(
        &self,
        hook: impl Fn(SessionId, &HandlerContext) + Send + Sync + 'static,
    ) {
        self.destroy_hooks.lock().unwrap().push(Box::new(hook));
    }

    /// Dispatch a command to its registered handler.
    ///
    /// Returns `true` if a handler was found, `false` otherwise.
    pub fn dispatch(&self, command: Command, ctx: &HandlerContext) -> bool {
        let key = command.key().to_string();
        self.handlers
            .read_sync(&key, |_, handler| {
                handler(command, ctx);
            })
            .is_some()
    }

    /// Run all session-destroy hooks for the given session.
    pub fn run_destroy_hooks(&self, session: SessionId, ctx: &HandlerContext) {
        for hook in self.destroy_hooks.lock().unwrap().iter() {
            hook(session, ctx);
        }
    }

    /// Check whether a handler is registered for a key.
    pub fn has_handler(&self, key: &str) -> bool {
        self.handlers.contains_sync(&key.to_string())
    }
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerRegistry")
            .field("handler_count", &self.handlers.len())
            .finish()
    }
}

/// Context provided to modules during initialization.
///
/// Borrows from FilerCore's internals so modules can register handlers,
/// spawn actors, and access shared infrastructure.
pub struct ModuleContext<'a> {
    /// Event sender — clone this into your actors
    pub events: Sender<Event>,
    /// Session manager — for session-aware modules
    pub sessions: &'a SessionManager,
    /// Node registry — for path ↔ NodeId resolution
    pub registry: &'a NodeRegistry,
    /// Actor system — spawn your actors here
    pub actors: &'a ActorSystem,
    /// Handler registry — register your command handlers here
    pub handlers: &'a HandlerRegistry,
}

/// A module bundles command handlers with the actors they dispatch to.
///
/// Modules are the unit of composition for filer-core. Each module:
/// 1. Creates its internal channels
/// 2. Registers command handlers via `ctx.handlers.on()`
/// 3. Optionally registers session-destroy hooks
/// 4. Spawns its actors via `ctx.actors.spawn()`
///
/// # Example
///
/// ```ignore
/// use filer_core::{Actor, Module};
/// use filer_core::api::module::ModuleContext;
///
/// struct MyModule;
///
/// impl Module for MyModule {
///     fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
///         let (tx, rx) = flume::unbounded();
///
///         ctx.handlers.on("my.command", move |cmd, _ctx| {
///             let _ = tx.send(cmd);
///         });
///
///         ctx.actors.spawn(MyActor::new(rx, ctx.events.clone()));
///     }
/// }
/// ```
pub trait Module: Send + 'static {
    /// Initialize the module: register handlers and spawn actors.
    ///
    /// Called once during [`FilerCore::load()`]. After this returns,
    /// the module is consumed and its actors are running.
    fn init(self: Box<Self>, ctx: ModuleContext<'_>);
}
