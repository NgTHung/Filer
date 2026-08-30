//! # Git file decorations
//!
//! This module computes semantic Git state for the visible rows supplied by a
//! client. It is opt-in because Git work is useful to file-manager clients but
//! should not add process or watch overhead to every core runtime.
//!
//! ```no_run
//! use std::sync::Arc;
//! use filer_core::{Command, FilerCore, Location, LocationRef, RequestId};
//! use filer_core::modules::git_decorations::{GitDecorationRequest, GitDecorationsModule};
//! use filer_core::model::session::SessionId;
//!
//! let core = FilerCore::new();
//! core.load(GitDecorationsModule::new());
//! let parent = LocationRef::from_location(&Location::local("/workspace"));
//! let request = GitDecorationRequest {
//!     parent,
//!     visible: Vec::new(),
//!     request: RequestId::new(),
//! };
//! let _ = core.send(Command::Extension {
//!     key: "git.status".to_string(),
//!     payload: Arc::new(request),
//!     session: SessionId::DEFAULT,
//! });
//! ```

mod actor;
mod backend;

pub use backend::{GitCliBackend, GitRepository, GitStatusBackend, GitStatusResult};

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::model::location::LocationRef;
use crate::model::request::RequestId;
use crate::utils::channel::send_or_warn;
use crate::vfs::local_watch::LocalWatchProvider;
use crate::vfs::watch::WatchProvider;

/// The maximum number of visible rows a single status request may contain.
pub const MAX_VISIBLE_DECORATIONS: usize = 1024;

/// In-process request sent through the `git.status` extension command.
#[derive(Debug, Clone)]
pub struct GitDecorationRequest {
    pub parent: LocationRef,
    pub visible: Vec<LocationRef>,
    pub request: RequestId,
}

/// One resolved visible row passed to a status backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDecorationTarget {
    pub location: LocationRef,
    pub path: std::path::PathBuf,
}

/// Semantic Git state for one file-manager row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileDecorationState {
    Modified,
    Added,
    Deleted,
    Untracked,
    Ignored,
    Conflicted,
    Clean,
}

/// A semantic decoration addressed by canonical location identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDecoration {
    pub location: LocationRef,
    pub state: FileDecorationState,
}

/// A batch of locations that must be recomputed by the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDecorationInvalidation {
    pub locations: Vec<LocationRef>,
}

pub(crate) use actor::{GitDecorationCommand, GitDecorationsActor};

/// Built-in Git decoration module.
pub struct GitDecorationsModule {
    backend: Arc<dyn GitStatusBackend>,
    watch_provider: Arc<dyn WatchProvider>,
    command_tx: flume::Sender<GitDecorationCommand>,
    command_rx: Option<flume::Receiver<GitDecorationCommand>>,
}

impl GitDecorationsModule {
    /// Build the opt-in module backed by the local Git executable.
    pub fn new() -> Self {
        Self::with_components(
            Arc::new(GitCliBackend::new()),
            Arc::new(LocalWatchProvider::new()),
        )
    }

    /// Build the module with an injectable status backend.
    pub fn with_backend(backend: Arc<dyn GitStatusBackend>) -> Self {
        Self::with_components(backend, Arc::new(LocalWatchProvider::new()))
    }

    #[cfg(test)]
    pub(crate) fn with_components(
        backend: Arc<dyn GitStatusBackend>,
        watch_provider: Arc<dyn WatchProvider>,
    ) -> Self {
        let (command_tx, command_rx) = flume::unbounded();
        Self {
            backend,
            watch_provider,
            command_tx,
            command_rx: Some(command_rx),
        }
    }

    #[cfg(not(test))]
    fn with_components(
        backend: Arc<dyn GitStatusBackend>,
        watch_provider: Arc<dyn WatchProvider>,
    ) -> Self {
        let (command_tx, command_rx) = flume::unbounded();
        Self {
            backend,
            watch_provider,
            command_tx,
            command_rx: Some(command_rx),
        }
    }
}

impl Default for GitDecorationsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitDecorationsModule {
    fn init(mut self: Box<Self>, ctx: ModuleContext<'_>) {
        let Some(command_rx) = self.command_rx.take() else {
            tracing::error!("GitDecorationsModule was initialized more than once");
            return;
        };

        let command_tx = self.command_tx.clone();
        ctx.handlers.on("git.status", move |command, context| {
            let Command::Extension {
                key,
                payload,
                session,
            } = command
            else {
                return;
            };
            if key != "git.status" {
                return;
            }
            let Some(request) = payload.as_ref().downcast_ref::<GitDecorationRequest>() else {
                send_or_warn(
                    &context.events,
                    crate::Event::from_error(
                        crate::CoreError::invalid_input(
                            "git.status payload must be GitDecorationRequest",
                        ),
                        session,
                    ),
                    "git decoration payload error",
                );
                return;
            };
            if request.visible.len() > MAX_VISIBLE_DECORATIONS {
                send_or_warn(
                    &context.events,
                    crate::Event::from_request_error(
                        crate::CoreError::invalid_input(format!(
                            "git.status accepts at most {MAX_VISIBLE_DECORATIONS} visible locations"
                        )),
                        session,
                        request.request,
                    ),
                    "git decoration request limit error",
                );
                return;
            }
            send_or_warn(
                &command_tx,
                GitDecorationCommand::Status {
                    parent: request.parent.clone(),
                    visible: request.visible.clone(),
                    session,
                    request: request.request,
                },
                "git status",
            );
        });

        let command_tx = self.command_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _context| {
            send_or_warn(
                &command_tx,
                GitDecorationCommand::SessionDestroyed(session),
                "git decoration session cleanup",
            );
        });

        ctx.actors.spawn(
            GitDecorationsActor::new(
                command_rx,
                ctx.events.clone(),
                ctx.registry.clone(),
                self.backend,
                self.watch_provider,
            )
            .with_work_tracker(ctx.actors.work_tracker()),
        );
    }
}
