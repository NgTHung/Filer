//! Command Router — generic command dispatcher with session validation.
//!
//! The Router sits between the API layer and the actor system. It receives
//! `Command` from the FilerCore handle and dispatches them to handlers
//! registered in the [`HandlerRegistry`] by modules.
//!
//! ## Dispatch flow
//!
//! 1. Receive `Command` from API channel
//! 2. Validate session via `Command::session_id()` + `SessionManager`
//! 3. Look up handler by `Command::key()` in the `HandlerRegistry`
//! 4. Call the handler (which forwards to the appropriate actor channel)
//! 5. For `DestroySession`, run all registered destroy hooks
//!
//! ## Session lifecycle
//!
//! Session creation (Handshake) and destruction (DestroySession) are
//! registered as normal handlers during `FilerCore::new()`. Modules
//! register `on_session_destroy` hooks for per-module cleanup.
//!
//! ## The Router knows nothing about actors
//!
//! Unlike the previous design, the Router holds no actor-specific channels.
//! All routing knowledge lives in the handler closures registered by modules.
//! This makes the system open for extension without modifying the Router.

use std::sync::Arc;

use flume::Receiver;

use crate::actors::Actor;
use crate::api::commands::Command;
use crate::api::module::{HandlerContext, HandlerRegistry};
use crate::utils::channel::send_or_warn;

/// Command Router actor — generic dispatcher backed by [`HandlerRegistry`].
pub struct CommandRouter {
    /// Incoming high-level commands from the API
    commands: Receiver<Command>,
    /// Registered command handlers (shared with FilerCore)
    handlers: Arc<HandlerRegistry>,
    /// Shared context passed to every handler invocation
    ctx: HandlerContext,
}

impl CommandRouter {
    pub fn new(
        commands: Receiver<Command>,
        handlers: Arc<HandlerRegistry>,
        ctx: HandlerContext,
    ) -> Self {
        Self {
            commands,
            handlers,
            ctx,
        }
    }

    /// Route a single command.
    ///
    /// Session validation is the only cross-cutting concern the Router
    /// handles directly. Everything else is delegated to the registry.
    fn route(&self, command: Command) {
        // ── Session validation ───────────────────────────────────────
        if let Some(session) = command.session_id()
            && !self.ctx.sessions.exists(session)
        {
            send_or_warn(
                &self.ctx.events,
                crate::api::events::Event::Error {
                    message: format!("Unknown session: {}", session),
                    recoverable: true,
                    session,
                },
                "unknown session error",
            );
            return;
        }

        // Check if this is a session-destroy (need session_id for hooks)
        let destroy_session = match &command {
            Command::DestroySession(s) => Some(*s),
            _ => None,
        };

        // Dispatch to registered handler
        let key = command.key().to_string();
        if !self.handlers.dispatch(command, &self.ctx) {
            tracing::warn!(key = %key, "no handler registered for command");
        }

        // Run destroy hooks after the session.destroy handler
        if let Some(session_id) = destroy_session {
            self.handlers.run_destroy_hooks(session_id, &self.ctx);
        }
    }
}

impl Actor for CommandRouter {
    async fn run(self) {
        while let Ok(command) = self.commands.recv_async().await {
            self.route(command);
        }
    }
    fn name(&self) -> &'static str {
        "command-router"
    }
}
