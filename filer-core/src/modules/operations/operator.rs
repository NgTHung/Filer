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
use crate::{CoreError, ErrorCode, ErrorTarget, FsProvider, ProviderCx};

use super::target::{affected_location, resolve_direct_target, resolve_direct_targets};

type TrashFn = Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>;

/// Build the provider context for an operation from its cancel token and an
/// optional deadline. Every operator provider call goes through `cx.race`, so
/// a cancel or breached deadline interrupts the in-flight call.
fn operation_cx(cancel: &CancellationToken, deadline: Option<Instant>) -> ProviderCx<'_> {
    let cx = ProviderCx::with_cancel(cancel);
    match deadline {
        Some(deadline) => cx.with_deadline(deadline),
        None => cx,
    }
}

#[derive(Debug, Clone)]
pub enum OpsCommand {
    Copy {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Move {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Delete {
        targets: Vec<LocationRef>,
        trash: bool,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Rename {
        source: LocationRef,
        new_name: String,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFolder {
        parent: LocationRef,
        name: String,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFile {
        parent: LocationRef,
        name: String,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Cancel(SessionId),
    CancelOperation {
        session: SessionId,
        operation: OperationId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationEventMode {
    Compat,
    Location,
}

pub struct Operator {
    commands: Receiver<OpsCommand>,
    events: EventSink,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_ops: CancelMap,
    active_operation_ids: Arc<scc::HashMap<SessionId, OperationId, RandomState>>,
    trash_fn: TrashFn,
    cache: Option<SharedDirCache>,
    default_timeout: Option<Duration>,
    work: WorkTracker,
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

    fn arm_operation(&self, session: SessionId, operation: OperationId) -> CancellationToken {
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

    fn copy(
        &self,
        sources: Vec<LocationRef>,
        dest: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        event_mode: OperationEventMode,
    ) {
        let dst_path = match resolve_direct_target(
            &self.registry,
            &dest,
            OperationKind::Copy,
            self.provider.capabilities(),
        ) {
            Ok(path) => path,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: copy resolve dest",
                );
                return;
            }
        };

        let src_paths = match resolve_direct_targets(
            &self.registry,
            &sources,
            OperationKind::Copy,
            self.provider.capabilities(),
        ) {
            Ok(paths) => paths,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: copy resolve src",
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
            let mut affected = Vec::new();
            let mut items_done = 0usize;

            for src_path in src_paths {
                if cancel.is_cancelled() {
                    emit_operation_progress(
                        &events,
                        OperationKind::Copy,
                        session,
                        request,
                        operation,
                        ProgressSnapshot::new(
                            ProgressStatus::Cancelled,
                            ProgressPhase::Processing,
                            ProgressUnit::Item,
                            affected.len(),
                            None,
                            None,
                        ),
                    )
                    .await;
                    return;
                }

                let meta = match cx.race(fs.scheme(), fs.metadata(&src_path, &cx)).await {
                    Ok(meta) => meta,
                    Err(e) => {
                        send_or_warn_async(
                            &events,
                            operation_error(e, session, request, operation),
                            "operator: copy stat",
                        )
                        .await;
                        return;
                    }
                };

                let file_name = src_path.file_name().unwrap_or_default();

                if meta.is_dir() {
                    let dst_sub = dst_path.join(file_name);
                    match copy_dir_recursive(
                        &fs,
                        &src_path,
                        &dst_sub,
                        &cx,
                        &events,
                        &registry,
                        session,
                        operation,
                        request,
                        &mut items_done,
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(e) if e.code() == ErrorCode::Cancelled => {
                            emit_operation_progress(
                                &events,
                                OperationKind::Copy,
                                session,
                                request,
                                operation,
                                ProgressSnapshot::new(
                                    ProgressStatus::Cancelled,
                                    ProgressPhase::Processing,
                                    ProgressUnit::Item,
                                    items_done,
                                    None,
                                    None,
                                ),
                            )
                            .await;
                            return;
                        }
                        Err(e) => {
                            send_or_warn_async(
                                &events,
                                operation_error(e, session, request, operation),
                                "operator: copy dir",
                            )
                            .await;
                            return;
                        }
                    }
                    invalidate_parent_cache(&cache, &dst_sub);
                    affected.push(affected_location(&registry, dst_sub));
                } else {
                    let dst_file = dst_path.join(file_name);
                    if let Err(e) = cx
                        .race(fs.scheme(), fs.copy(&src_path, &dst_file, &cx))
                        .await
                    {
                        send_or_warn_async(
                            &events,
                            operation_error(e, session, request, operation),
                            "operator: copy file",
                        )
                        .await;
                        return;
                    }
                    invalidate_parent_cache(&cache, &dst_file);
                    items_done += 1;
                    affected.push(affected_location(&registry, dst_file));
                }
            }

            emit_operation_progress(
                &events,
                OperationKind::Copy,
                session,
                request,
                operation,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Item,
                    items_done,
                    None,
                    None,
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                match operation_complete_event(
                    &registry,
                    OperationKind::Copy,
                    operation,
                    affected,
                    session,
                    event_mode,
                ) {
                    Ok(event) => event,
                    Err(error) => operation_error(error, session, request, operation),
                },
                "operator: copy complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn moves(
        &self,
        sources: Vec<LocationRef>,
        dest: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        event_mode: OperationEventMode,
    ) {
        let dst_path = match resolve_direct_target(
            &self.registry,
            &dest,
            OperationKind::Move,
            self.provider.capabilities(),
        ) {
            Ok(path) => path,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: move resolve dest",
                );
                return;
            }
        };

        let src_paths = match resolve_direct_targets(
            &self.registry,
            &sources,
            OperationKind::Move,
            self.provider.capabilities(),
        ) {
            Ok(paths) => paths,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    operation_error(error, session, request, operation),
                    "operator: move resolve src",
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
            let mut affected = Vec::new();

            for src_path in src_paths {
                if cancel.is_cancelled() {
                    emit_operation_progress(
                        &events,
                        OperationKind::Move,
                        session,
                        request,
                        operation,
                        ProgressSnapshot::new(
                            ProgressStatus::Cancelled,
                            ProgressPhase::Processing,
                            ProgressUnit::Item,
                            affected.len(),
                            None,
                            None,
                        ),
                    )
                    .await;
                    return;
                }

                let file_name = src_path.file_name().unwrap_or_default();
                let dst_file = dst_path.join(file_name);

                match cx
                    .race(fs.scheme(), fs.rename(&src_path, &dst_file, &cx))
                    .await
                {
                    Ok(()) => {
                        invalidate_parent_cache(&cache, &src_path);
                        invalidate_parent_cache(&cache, &dst_file);
                        invalidate_subtree_cache(&cache, &src_path);
                        affected.push(affected_location(&registry, dst_file));
                    }
                    Err(e) if is_cross_device(&e) => {
                        if let Err(e) = cx
                            .race(fs.scheme(), fs.copy(&src_path, &dst_file, &cx))
                            .await
                        {
                            send_or_warn_async(
                                &events,
                                operation_error(e, session, request, operation),
                                "operator: move copy",
                            )
                            .await;
                            return;
                        }
                        if let Err(e) = cx.race(fs.scheme(), fs.delete(&src_path, &cx)).await {
                            send_or_warn_async(
                                &events,
                                operation_error(e, session, request, operation),
                                "operator: move delete",
                            )
                            .await;
                            return;
                        }
                        invalidate_parent_cache(&cache, &src_path);
                        invalidate_parent_cache(&cache, &dst_file);
                        invalidate_subtree_cache(&cache, &src_path);
                        affected.push(affected_location(&registry, dst_file));
                    }
                    Err(e) => {
                        send_or_warn_async(
                            &events,
                            operation_error(e, session, request, operation),
                            "operator: move rename",
                        )
                        .await;
                        return;
                    }
                }
            }

            emit_operation_progress(
                &events,
                OperationKind::Move,
                session,
                request,
                operation,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Item,
                    affected.len(),
                    None,
                    None,
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                match operation_complete_event(
                    &registry,
                    OperationKind::Move,
                    operation,
                    affected,
                    session,
                    event_mode,
                ) {
                    Ok(event) => event,
                    Err(error) => operation_error(error, session, request, operation),
                },
                "operator: move complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
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

async fn copy_dir_recursive(
    fs: &Arc<dyn FsProvider>,
    src: &Path,
    dst: &Path,
    cx: &ProviderCx<'_>,
    events: &EventSink,
    registry: &NodeRegistry,
    session: SessionId,
    operation: OperationId,
    request: RequestId,
    items_done: &mut usize,
) -> Result<(), CoreError> {
    cx.race(fs.scheme(), fs.mkdir(dst, cx)).await?;
    let entries = cx.race(fs.scheme(), fs.list(src, cx)).await?;
    for entry in entries {
        if cx.cancel.is_some_and(crate::CancelSignal::is_cancelled) {
            return Err(CoreError::cancelled());
        }
        let src_child = src.join(&entry.name);
        let dst_child = dst.join(&entry.name);
        if entry.is_dir() {
            Box::pin(copy_dir_recursive(
                fs, &src_child, &dst_child, cx, events, registry, session, operation, request,
                items_done,
            ))
            .await?;
        } else {
            cx.race(fs.scheme(), fs.copy(&src_child, &dst_child, cx))
                .await?;
            *items_done += 1;
            let id = registry.clone().register(dst_child);
            send_or_warn_async(
                events,
                Event::ProgressUpdated {
                    scope: ProgressScope::operation(
                        OperationKind::Copy,
                        session,
                        request,
                        operation,
                    ),
                    snapshot: ProgressSnapshot::new(
                        ProgressStatus::Running,
                        ProgressPhase::Processing,
                        ProgressUnit::Item,
                        *items_done,
                        None,
                        Some(ProgressTarget::Node(id)),
                    ),
                },
                "operator: copy dir progress",
            )
            .await;
        }
    }
    Ok(())
}

fn operation_error(
    err: CoreError,
    session: SessionId,
    request: RequestId,
    operation: OperationId,
) -> Event {
    Event::from_operation_error(err, session, request, operation)
}

fn operation_complete_event(
    registry: &NodeRegistry,
    kind: OperationKind,
    operation: OperationId,
    affected: Vec<LocationRef>,
    session: SessionId,
    event_mode: OperationEventMode,
) -> Result<Event, CoreError> {
    match event_mode {
        OperationEventMode::Compat => {
            let affected = affected
                .into_iter()
                .map(|location| {
                    registry
                        .resolve_location_ref(&location)
                        .and_then(|location| registry.register_location_node(location))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Event::OperationCompleteCompat {
                operation_id: operation,
                operation: kind,
                success: true,
                affected,
                session,
            })
        }
        OperationEventMode::Location => Ok(Event::OperationComplete {
            operation_id: operation,
            operation: kind,
            success: true,
            affected,
            session,
        }),
    }
}

async fn emit_operation_progress(
    events: &EventSink,
    kind: OperationKind,
    session: SessionId,
    request: RequestId,
    operation: OperationId,
    snapshot: ProgressSnapshot,
) {
    send_or_warn_async(
        events,
        Event::ProgressUpdated {
            scope: ProgressScope::operation(kind, session, request, operation),
            snapshot,
        },
        "operator: progress",
    )
    .await;
}

fn is_cross_device(err: &CoreError) -> bool {
    err.code() == ErrorCode::IoFailed
        && (err.message.contains("cross-device")
            || err.message.contains("os error 18")
            || err.message.contains("os error 17"))
}

fn invalidate_parent_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let (Some(parent), Some(c)) = (path.parent(), cache) {
        if let Ok(mut guard) = c.lock() {
            guard.invalidate(parent);
        }
    }
}

fn invalidate_subtree_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let Some(c) = cache {
        if let Ok(mut guard) = c.lock() {
            guard.invalidate_subtree(path);
        }
    }
}

async fn remove_operation_if_current(
    active_operation_ids: Arc<scc::HashMap<SessionId, OperationId, RandomState>>,
    session: SessionId,
    operation: OperationId,
) {
    let _ = active_operation_ids
        .remove_if_async(&session, |current| *current == operation)
        .await;
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
