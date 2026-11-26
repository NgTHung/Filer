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

use rapidhash::fast::RandomState;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::actors::ActorSystem;
use crate::api::commands::Command;
use crate::api::event_sink::EventSink;
use crate::api::session_manager::SessionManager;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;

/// Context available to command handlers during dispatch.
///
/// Passed by reference to every handler invocation. Provides access
/// to the event bus, session manager, and Location registry.
#[derive(Clone)]
pub struct HandlerContext {
    pub events: EventSink,
    pub sessions: SessionManager,
    pub registry: NodeRegistry,
}

type HandlerFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
type SyncHandler = dyn Fn(Command, &HandlerContext) + Send + Sync;
type AsyncHandler = dyn for<'a> Fn(Command, &'a HandlerContext) -> HandlerFuture<'a> + Send + Sync;

enum HandlerFn {
    Sync(Box<SyncHandler>),
    Async(Box<AsyncHandler>),
}

struct Handler {
    callback: HandlerFn,
    builtin_only: bool,
}

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
/// If no handler accepts the command, the Router reports a correlated error.
pub struct HandlerRegistry {
    handlers: scc::HashMap<String, Arc<Handler>, RandomState>,
    destroy_hooks: std::sync::Mutex<Vec<DestroyHookFn>>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: scc::HashMap::with_hasher(RandomState::new()),
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
        self.register(key.into(), HandlerFn::Sync(Box::new(handler)), false);
    }

    pub(crate) fn on_builtin(
        &self,
        key: &str,
        handler: impl Fn(Command, &HandlerContext) + Send + Sync + 'static,
    ) {
        self.register(key.into(), HandlerFn::Sync(Box::new(handler)), true);
    }

    /// Register a handler that awaits capacity when delivering lossless events.
    pub fn on_async(
        &self,
        key: impl Into<String>,
        handler: impl for<'a> Fn(Command, &'a HandlerContext) -> HandlerFuture<'a>
        + Send
        + Sync
        + 'static,
    ) {
        self.register(key.into(), HandlerFn::Async(Box::new(handler)), false);
    }

    pub(crate) fn on_builtin_async(
        &self,
        key: &str,
        handler: impl for<'a> Fn(Command, &'a HandlerContext) -> HandlerFuture<'a>
        + Send
        + Sync
        + 'static,
    ) {
        self.register(key.into(), HandlerFn::Async(Box::new(handler)), true);
    }

    fn register(&self, key: String, callback: HandlerFn, builtin_only: bool) {
        let _ = self.handlers.upsert_sync(
            key,
            Arc::new(Handler {
                callback,
                builtin_only,
            }),
        );
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
    /// Returns `true` if a handler accepted the command for dispatch.
    pub async fn dispatch(&self, command: Command, ctx: &HandlerContext) -> bool {
        self.dispatch_or_return(command, ctx).await.is_none()
    }

    pub(crate) async fn dispatch_or_return(
        &self,
        command: Command,
        ctx: &HandlerContext,
    ) -> Option<Command> {
        // Await delivery after releasing the registry guard so registration can proceed.
        let Some(handler) = self
            .handlers
            .read_sync(command.key(), |_, handler| Arc::clone(handler))
        else {
            return Some(command);
        };
        if handler.builtin_only && matches!(command, Command::Extension { .. }) {
            return Some(command);
        }
        match &handler.callback {
            HandlerFn::Sync(handler) => handler(command, ctx),
            HandlerFn::Async(handler) => handler(command, ctx).await,
        }
        None
    }

    /// Run all session-destroy hooks for the given session.
    pub fn run_destroy_hooks(&self, session: SessionId, ctx: &HandlerContext) {
        for hook in self.destroy_hooks.lock().unwrap().iter() {
            hook(session, ctx);
        }
    }

    /// Check whether a handler is registered for a key.
    pub fn has_handler(&self, key: &str) -> bool {
        self.handlers.contains_sync(key)
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
    /// Bounded event sink. Progress updates coalesce by scope while terminal,
    /// page, and change events remain lossless.
    pub events: EventSink,
    /// Session manager — for session-aware modules
    pub sessions: &'a SessionManager,
    /// Location registry for descriptor and route resolution.
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
