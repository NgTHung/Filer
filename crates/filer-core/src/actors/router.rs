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
use crate::api::events::Event;
use crate::api::module::{HandlerContext, HandlerRegistry};
use crate::errors::CoreError;
use crate::model::session::SessionId;
use crate::utils::channel::send_or_warn_async;

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
    async fn route(&self, command: Command) {
        let session = command.session_id();
        if let Some(session) = session
            && !self.ctx.sessions.exists(session)
        {
            self.reject(&command, session, CoreError::unknown_session(session))
                .await;
            return;
        }

        // Check if this is a session-destroy (need session_id for hooks)
        let destroy_session = match &command {
            Command::DestroySession(s) => Some(*s),
            _ => None,
        };

        // Trace dispatch attempts after session validation to keep command diagnostics in one place.
        tracing::debug!(
            target: "filer_core::command",
            key = command.key(),
            session = ?session,
            request = ?command.request_id(),
            operation = ?command.operation_id(),
            "command dispatched",
        );

        if let Some(command) = self.handlers.dispatch_or_return(command, &self.ctx).await {
            if let Some(session) = session {
                let error = CoreError::unsupported_operation(format!(
                    "No handler available for command '{}'",
                    command.key()
                ));
                self.reject(&command, session, error).await;
            } else {
                // FilerCore always installs the sessionless Handshake handler.
                tracing::error!(key = command.key(), "missing core lifecycle handler");
            }
            return;
        }

        // Run destroy hooks after the session.destroy handler
        if let Some(session_id) = destroy_session {
            self.handlers.run_destroy_hooks(session_id, &self.ctx);
        }
    }

    async fn reject(&self, command: &Command, session: SessionId, error: CoreError) {
        let event = Event::from_correlated_error(
            error,
            session,
            command.request_id(),
            command.operation_id(),
        );
        send_or_warn_async(&self.ctx.events, event, "command rejected").await;
    }
}

impl Actor for CommandRouter {
    async fn run(self) {
        while let Ok(command) = self.commands.recv_async().await {
            self.route(command).await;
        }
    }
    fn name(&self) -> &'static str {
        "command-router"
    }
}
