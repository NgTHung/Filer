//! # Command router
//!
//! The router validates Session admission before dispatch. It keeps recovery
//! commands routable while separate tasks await Session closure.
//!
//! ```no_run
//! # async fn example() -> Result<(), filer_core::CoreError> {
//! let core = filer_core::FilerCore::new();
//! let request = core.next_request_id();
//! core.send(filer_core::Command::Handshake { request })?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashSet;
use std::sync::Arc;

use flume::Receiver;
use tokio::task::JoinSet;

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
        if let Some(session) = session {
            match self.ctx.sessions.is_closing(session) {
                None => {
                    self.reject(&command, session, CoreError::unknown_session(session))
                        .await;
                    return;
                }
                Some(true)
                    if !matches!(
                        &command,
                        Command::CancelOperation { .. }
                            | Command::InspectOperationQueue { .. }
                            | Command::ResolveOperationQueue { .. }
                    ) =>
                {
                    self.reject(&command, session, CoreError::session_closing(session))
                        .await;
                    return;
                }
                Some(_) => {}
            }
        }

        Self::trace_dispatch(&command);

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
        }
    }

    fn trace_dispatch(command: &Command) {
        tracing::debug!(
            target: "filer_core::command",
            key = command.key(),
            session = ?command.session_id(),
            request = ?command.request_id(),
            operation = ?command.operation_id(),
            "command dispatched",
        );
    }

    fn start_close(
        &self,
        session: SessionId,
        closing: &mut HashSet<SessionId>,
        tasks: &mut JoinSet<(SessionId, Result<(), CoreError>)>,
    ) {
        if !closing.insert(session) {
            return;
        }
        let Some(work) = self.ctx.sessions.begin_close(session) else {
            closing.remove(&session);
            return;
        };
        let handlers = self.handlers.clone();
        tasks.spawn(async move {
            let cleanup = async {
                work.reads.wait().await;
                handlers.cleanup_session(session).await
            };
            let (cleanup, drain) = tokio::join!(cleanup, handlers.drain_session(session));
            work.mutations.wait().await;
            (session, cleanup.and(drain))
        });
    }

    async fn finish_close(&self, session: SessionId) -> Result<(), CoreError> {
        // A second fence orders controls routed while the first drain was completing.
        self.handlers.drain_session(session).await?;
        self.handlers.run_destroy_hooks(session, &self.ctx);
        self.ctx.sessions.remove(session);
        self.ctx.events.discard_session_progress(session);
        self.ctx
            .events
            .send_async(Event::SessionDestroyed(session))
            .await
            .map_err(|_| CoreError::channel_closed("events"))
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
        let mut closing = HashSet::new();
        let mut tasks = JoinSet::new();
        loop {
            tokio::select! {
                command = self.commands.recv_async() => match command {
                    Ok(Command::DestroySession(session)) if self.ctx.sessions.exists(session) => {
                        Self::trace_dispatch(&Command::DestroySession(session));
                        self.start_close(session, &mut closing, &mut tasks);
                    }
                    Ok(command) => self.route(command).await,
                    Err(_) => break,
                },
                Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                    match result {
                        Ok((session, result)) => {
                            closing.remove(&session);
                            let result = match result {
                                Ok(()) => self.finish_close(session).await,
                                Err(error) => Err(error),
                            };
                            if let Err(error) = result {
                                self.reject(&Command::DestroySession(session), session, error).await;
                            }
                        }
                        Err(error) => tracing::error!(%error, "Session closure task failed"),
                    }
                }
            }
        }
    }
    fn name(&self) -> &'static str {
        "command-router"
    }
}
