use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::Receiver;
use rapidhash::fast::RandomState;

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::location::LocationRef;
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::{CoreError, ErrorTarget, FsProvider};

pub use super::command::{OperationEventMode, OpsCommand};
pub(crate) use super::support::TrashFn;
use super::support::*;
use super::target::{affected_location, resolve_direct_target};

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

    #[allow(dead_code)]
    fn invalidate_parent(&self, path: &Path) {
        invalidate_parent_cache(&self.cache, path);
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

    fn delete(
        &self,
        targets: Vec<LocationRef>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        event_mode: OperationEventMode,
    ) {
        let mut paths: Vec<(LocationRef, PathBuf)> = Vec::new();
        for target in targets {
            let path = match resolve_direct_target(
                &self.registry,
                &target,
                OperationKind::Delete,
                self.provider.capabilities(),
            ) {
                Ok(path) => path,
                Err(error) => {
                    send_or_warn(
                        &self.events,
                        operation_error(error, session, request, operation),
                        "operator: delete resolve",
                    );
                    return;
                }
            };
            paths.push((target, path));
        }

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let fs = self.provider.clone();
        let trash_fn = self.trash_fn.clone();
        let cache = self.cache.clone();
        let registry = self.registry.clone();
        let total = paths.len();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            let cx = operation_cx(&cancel, deadline);
            let mut affected = Vec::new();
            let mut items_done = 0usize;

            for (location, path) in paths {
                if cancel.is_cancelled() {
                    emit_operation_progress(
                        &events,
                        OperationKind::Delete,
                        session,
                        request,
                        operation,
                        ProgressSnapshot::new(
                            ProgressStatus::Cancelled,
                            ProgressPhase::Processing,
                            ProgressUnit::Item,
                            items_done,
                            Some(total),
                            None,
                        ),
                    )
                    .await;
                    return;
                }

                let result = if trash {
                    let tf = trash_fn.clone();
                    let p = path.clone();
                    tokio::task::spawn_blocking(move || tf(&p))
                        .await
                        .unwrap_or_else(|e| Err(CoreError::actor("operator", e.to_string())))
                } else {
                    cx.race(fs.scheme(), fs.delete(&path, &cx)).await
                };

                match result {
                    Ok(()) => {
                        invalidate_parent_cache(&cache, &path);
                        invalidate_subtree_cache(&cache, &path);
                        affected.push(location.clone());
                        items_done += 1;
                        if total > 1 {
                            send_or_warn_async(
                                &events,
                                Event::ProgressUpdated {
                                    scope: ProgressScope::operation(
                                        OperationKind::Delete,
                                        session,
                                        request,
                                        operation,
                                    ),
                                    snapshot: ProgressSnapshot::new(
                                        ProgressStatus::Running,
                                        ProgressPhase::Processing,
                                        ProgressUnit::Item,
                                        items_done,
                                        Some(total),
                                        Some(ProgressTarget::Location(location.clone())),
                                    ),
                                },
                                "operator: delete progress",
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        send_or_warn_async(
                            &events,
                            operation_error(e, session, request, operation),
                            "operator: delete error",
                        )
                        .await;
                        return;
                    }
                }
            }

            emit_operation_progress(
                &events,
                OperationKind::Delete,
                session,
                request,
                operation,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Item,
                    items_done,
                    Some(total),
                    None,
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                match operation_complete_event(
                    &registry,
                    OperationKind::Delete,
                    operation,
                    affected,
                    session,
                    event_mode,
                ) {
                    Ok(event) => event,
                    Err(error) => operation_error(error, session, request, operation),
                },
                "operator: delete complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn rename(
        &self,
        source: LocationRef,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        event_mode: OperationEventMode,
    ) {
        let src_path = match resolve_direct_target(
            &self.registry,
            &source,
            OperationKind::Rename,
            self.provider.capabilities(),
        ) {
            Ok(path) => path,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: rename resolve",
                );
                return;
            }
        };

        let Some(parent) = src_path.parent() else {
            send_or_warn(
                &self.events,
                Event::from_operation_error(
                    CoreError::invalid_path(format!("Cannot get parent of {}", src_path.display())),
                    session,
                    request,
                    operation,
                ),
                "operator: rename parent",
            );
            return;
        };

        let new_path = parent.join(&new_name);
        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            let cx = operation_cx(&cancel, deadline);
            match cx.race(fs.scheme(), fs.exists(&new_path, &cx)).await {
                Ok(true) => {
                    send_or_warn_async(
                        &events,
                        Event::from_operation_error(
                            CoreError::collision(
                                ErrorTarget::Path(src_path.clone()),
                                ErrorTarget::Path(new_path.clone()),
                            ),
                            session,
                            request,
                            operation,
                        ),
                        "operator: rename collision",
                    )
                    .await;
                    return;
                }
                Ok(false) => {}
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        operation_error(e, session, request, operation),
                        "operator: rename exists",
                    )
                    .await;
                    return;
                }
            }

            if let Err(e) = cx
                .race(fs.scheme(), fs.rename(&src_path, &new_path, &cx))
                .await
            {
                send_or_warn_async(
                    &events,
                    operation_error(e, session, request, operation),
                    "operator: rename",
                )
                .await;
                return;
            }

            invalidate_parent_cache(&cache, &src_path);
            invalidate_subtree_cache(&cache, &src_path);
            let location = affected_location(&registry, new_path);
            emit_operation_progress(
                &events,
                OperationKind::Rename,
                session,
                request,
                operation,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Item,
                    1,
                    Some(1),
                    Some(ProgressTarget::Location(location.clone())),
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                match operation_complete_event(
                    &registry,
                    OperationKind::Rename,
                    operation,
                    vec![location],
                    session,
                    event_mode,
                ) {
                    Ok(event) => event,
                    Err(error) => operation_error(error, session, request, operation),
                },
                "operator: rename complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn create_file(
        &self,
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        event_mode: OperationEventMode,
    ) {
        let path = match resolve_direct_target(
            &self.registry,
            &parent,
            OperationKind::CreateFile,
            self.provider.capabilities(),
        ) {
            Ok(path) => path,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: create_file resolve",
                );
                return;
            }
        };

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            let cx = operation_cx(&cancel, deadline);
            let full_path = path.join(name);
            match cx.race(fs.scheme(), fs.exists(&full_path, &cx)).await {
                Ok(true) => {
                    send_or_warn_async(
                        &events,
                        Event::from_operation_error(
                            CoreError::collision(
                                ErrorTarget::Path(path.clone()),
                                ErrorTarget::Path(full_path.clone()),
                            ),
                            session,
                            request,
                            operation,
                        ),
                        "operator: create_file exists",
                    )
                    .await;
                    return;
                }
                Ok(false) => {}
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        operation_error(e, session, request, operation),
                        "operator: create_file exists",
                    )
                    .await;
                    return;
                }
            }
            if let Err(e) = cx.race(fs.scheme(), fs.write(&full_path, &[], &cx)).await {
                send_or_warn_async(
                    &events,
                    operation_error(e, session, request, operation),
                    "operator: create_file write",
                )
                .await;
                return;
            }
            invalidate_parent_cache(&cache, &full_path);
            let location = affected_location(&registry, full_path);
            emit_operation_progress(
                &events,
                OperationKind::CreateFile,
                session,
                request,
                operation,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Item,
                    1,
                    Some(1),
                    Some(ProgressTarget::Location(location.clone())),
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                match operation_complete_event(
                    &registry,
                    OperationKind::CreateFile,
                    operation,
                    vec![location],
                    session,
                    event_mode,
                ) {
                    Ok(event) => event,
                    Err(error) => operation_error(error, session, request, operation),
                },
                "operator: create_file complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn create_folder(
        &self,
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        event_mode: OperationEventMode,
    ) {
        let path = match resolve_direct_target(
            &self.registry,
            &parent,
            OperationKind::CreateFolder,
            self.provider.capabilities(),
        ) {
            Ok(path) => path,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: create_folder resolve",
                );
                return;
            }
        };

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            let cx = operation_cx(&cancel, deadline);
            let full_path = path.join(name);
            match cx.race(fs.scheme(), fs.exists(&full_path, &cx)).await {
                Ok(true) => {
                    send_or_warn_async(
                        &events,
                        Event::from_operation_error(
                            CoreError::collision(
                                ErrorTarget::Path(path.clone()),
                                ErrorTarget::Path(full_path.clone()),
                            ),
                            session,
                            request,
                            operation,
                        ),
                        "operator: create_folder exists",
                    )
                    .await;
                    return;
                }
                Ok(false) => {}
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        operation_error(e, session, request, operation),
                        "operator: create_folder exists",
                    )
                    .await;
                    return;
                }
            }
            if let Err(e) = cx.race(fs.scheme(), fs.mkdir(&full_path, &cx)).await {
                send_or_warn_async(
                    &events,
                    operation_error(e, session, request, operation),
                    "operator: create_folder mkdir",
                )
                .await;
                return;
            }
            invalidate_parent_cache(&cache, &full_path);
            let location = affected_location(&registry, full_path);
            emit_operation_progress(
                &events,
                OperationKind::CreateFolder,
                session,
                request,
                operation,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Item,
                    1,
                    Some(1),
                    Some(ProgressTarget::Location(location.clone())),
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                match operation_complete_event(
                    &registry,
                    OperationKind::CreateFolder,
                    operation,
                    vec![location],
                    session,
                    event_mode,
                ) {
                    Ok(event) => event,
                    Err(error) => operation_error(error, session, request, operation),
                },
                "operator: create_folder complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
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
                }) => self.copy(
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                    event_mode,
                ),
                Ok(OpsCommand::Move {
                    sources,
                    destination,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => self.moves(
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                    event_mode,
                ),
                Ok(OpsCommand::Delete {
                    targets,
                    trash,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => self.delete(targets, trash, session, request, operation, event_mode),
                Ok(OpsCommand::Rename {
                    source,
                    new_name,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => self.rename(source, new_name, session, request, operation, event_mode),
                Ok(OpsCommand::CreateFile {
                    parent,
                    name,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => self.create_file(parent, name, session, request, operation, event_mode),
                Ok(OpsCommand::CreateFolder {
                    parent,
                    name,
                    event_mode,
                    session,
                    request,
                    operation,
                }) => self.create_folder(parent, name, session, request, operation, event_mode),
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}
