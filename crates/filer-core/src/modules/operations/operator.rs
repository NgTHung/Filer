//! # Operation actor
//!
//! The operation actor owns admission and cancellation state, then dispatches
//! accepted commands to focused transfer and mutation handlers. Command types
//! remain re-exported here so existing callers keep a stable import path.
//!
//! ```
//! use filer_core::model::session::SessionId;
//! use filer_core::modules::operations::operator::OpsCommand;
//! use filer_core::OperationId;
//!
//! let command = OpsCommand::CancelOperation {
//!     session: SessionId(1),
//!     operation: OperationId(2),
//! };
//! assert!(matches!(command, OpsCommand::CancelOperation { .. }));
//! ```

use std::sync::Arc;
use std::time::Duration;

use flume::Receiver;
use rapidhash::fast::RandomState;

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::model::operation::OperationId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::{CoreError, FsProvider};

pub use super::command::{OperationEventMode, OpsCommand};
pub(crate) use super::support::TrashFn;

pub struct Operator {
    commands: Receiver<OpsCommand>,
    pub(super) events: EventSink,
    pub(super) provider: Arc<dyn FsProvider>,
    pub(super) registry: NodeRegistry,
    pub(super) active_ops: CancelMap,
    pub(super) active_operation_ids: Arc<scc::HashMap<SessionId, OperationId, RandomState>>,
    pub(super) trash_fn: TrashFn,
    pub(super) cache: Option<SharedDirCache>,
    pub(super) default_timeout: Option<Duration>,
    pub(super) work: WorkTracker,
}

impl Operator {
    pub fn new<E: Into<EventSink>>(
        commands: Receiver<OpsCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self::with_trash_fn(
            commands,
            events.into(),
            provider,
            registry,
            Arc::new(|path| {
                trash::delete(path).map_err(|e| CoreError::io(path.to_path_buf(), e.to_string()))
            }),
        )
    }

    pub fn with_trash_fn<E: Into<EventSink>>(
        commands: Receiver<OpsCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        trash_fn: TrashFn,
    ) -> Self {
        Self {
            commands,
            events: events.into(),
            provider,
            registry,
            active_ops: CancelMap::new(),
            active_operation_ids: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            trash_fn,
            cache: None,
            default_timeout: None,
            work: WorkTracker::new(),
        }
    }

    /// Bound each provider call during an operation to `timeout`.
    ///
    /// `None` leaves operations unbounded. A breached deadline ends the
    /// operation with a `TimedOut` error carrying provider context.
    pub fn set_operation_timeout(&mut self, timeout: Option<Duration>) {
        self.default_timeout = timeout;
    }

    pub fn with_cache<E: Into<EventSink>>(
        commands: Receiver<OpsCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        let mut op = Self::new(commands, events, provider, registry);
        op.cache = Some(cache);
        op
    }

    pub(crate) fn with_work_tracker(mut self, work: WorkTracker) -> Self {
        self.work = work;
        self
    }

    pub(super) fn arm_operation(
        &self,
        session: SessionId,
        operation: OperationId,
    ) -> CancellationToken {
        let _ = self.active_operation_ids.remove_sync(&session);
        let _ = self.active_operation_ids.insert_sync(session, operation);
        self.active_ops.arm(session)
    }

    fn cancel_operation(&self, session: SessionId, operation: OperationId) {
        let active = self
            .active_operation_ids
            .read_sync(&session, |_, current| *current == operation)
            .unwrap_or(false);
        if active {
            self.active_ops.cancel(session);
            let _ = self.active_operation_ids.remove_sync(&session);
        }
    }

    fn cancel_session(&self, session: SessionId) {
        self.active_ops.cancel(session);
        let _ = self.active_operation_ids.remove_sync(&session);
    }
}

impl Actor for Operator {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Err(_) => {
                    self.active_ops.cancel_all().await;
                    break;
                }
                Ok(OpsCommand::Cancel(s)) => self.cancel_session(s),
                Ok(OpsCommand::CancelOperation { session, operation }) => {
                    self.cancel_operation(session, operation);
                }
                Ok(OpsCommand::Copy {
                    sources,
                    destination,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => {
                    self.copy(
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                        event_mode,
                    )
                    .await
                }
                Ok(OpsCommand::Move {
                    sources,
                    destination,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => {
                    self.moves(
                        sources,
                        destination,
                        session,
                        request,
                        operation,
                        event_mode,
                    )
                    .await
                }
                Ok(OpsCommand::Delete {
                    targets,
                    trash,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => {
                    self.delete(targets, trash, session, request, operation, event_mode)
                        .await
                }
                Ok(OpsCommand::Rename {
                    source,
                    new_name,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => {
                    self.rename(source, new_name, session, request, operation, event_mode)
                        .await
                }
                Ok(OpsCommand::CreateFile {
                    parent,
                    name,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => {
                    self.create_file(parent, name, session, request, operation, event_mode)
                        .await
                }
                Ok(OpsCommand::CreateFolder {
                    parent,
                    name,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => {
                    self.create_folder(parent, name, session, request, operation, event_mode)
                        .await
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}
