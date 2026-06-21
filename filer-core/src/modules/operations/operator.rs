use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::Event;
use crate::model::capability::{LocationCapabilityError, operation_capability_for_location};
use crate::model::location::LocationRef;
use crate::model::node::NodeId;
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
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CopyLocation {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    MoveLocation {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Delete {
        targets: Vec<NodeId>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    DeleteLocation {
        targets: Vec<LocationRef>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Rename {
        source: NodeId,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    RenameLocation {
        source: LocationRef,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFolderLocation {
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFileLocation {
        parent: LocationRef,
        name: String,
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
enum CompletionShape {
    Node,
    Location,
}

pub struct Operator {
    commands: Receiver<OpsCommand>,
    events: Sender<Event>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_ops: CancelMap,
    active_operation_ids: Arc<scc::HashMap<SessionId, OperationId, RandomState>>,
    trash_fn: TrashFn,
    cache: Option<SharedDirCache>,
    default_timeout: Option<Duration>,
}

impl Operator {
    pub fn new(
        commands: Receiver<OpsCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self::with_trash_fn(
            commands,
            events,
            provider,
            registry,
            Arc::new(|path| {
                trash::delete(path).map_err(|e| CoreError::io(path.to_path_buf(), e.to_string()))
            }),
        )
    }

    pub fn with_trash_fn(
        commands: Receiver<OpsCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        trash_fn: TrashFn,
    ) -> Self {
        Self {
            commands,
            events,
            provider,
            registry,
            active_ops: CancelMap::new(),
            active_operation_ids: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            trash_fn,
            cache: None,
            default_timeout: None,
        }
    }

    /// Bound each provider call during an operation to `timeout`.
    ///
    /// `None` leaves operations unbounded. A breached deadline ends the
    /// operation with a `TimedOut` error carrying provider context.
    pub fn set_operation_timeout(&mut self, timeout: Option<Duration>) {
        self.default_timeout = timeout;
    }

    pub fn with_cache(
        commands: Receiver<OpsCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        let mut op = Self::new(commands, events, provider, registry);
        op.cache = Some(cache);
        op
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
        sources: Vec<NodeId>,
        dest: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        completion: CompletionShape,
    ) {
        let Some(dst_path) = self.registry.resolve(dest) else {
            send_or_warn(
                &self.events,
                Event::from_operation_error(
                    CoreError::invalid_input(format!("Cannot resolve destination {dest:?}")),
                    session,
                    request,
                    operation,
                ),
                "operator: copy resolve dest",
            );
            return;
        };

        let mut src_paths = Vec::new();
        for src_id in &sources {
            let Some(path) = self.registry.resolve(*src_id) else {
                send_or_warn(
                    &self.events,
                    Event::from_operation_error(
                        CoreError::invalid_input(format!("Cannot resolve source {src_id:?}")),
                        session,
                        request,
                        operation,
                    ),
                    "operator: copy resolve src",
                );
                return;
            };
            src_paths.push(path);
        }

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
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
                    affected.push(registry.clone().register(dst_sub));
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
                    affected.push(registry.clone().register(dst_file));
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
                operation_complete_event(
                    &registry,
                    OperationKind::Copy,
                    operation,
                    affected,
                    session,
                    completion,
                ),
                "operator: copy complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn moves(
        &self,
        sources: Vec<NodeId>,
        dest: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        completion: CompletionShape,
    ) {
        let Some(dst_path) = self.registry.resolve(dest) else {
            send_or_warn(
                &self.events,
                Event::from_operation_error(
                    CoreError::invalid_input(format!("Cannot resolve destination {dest:?}")),
                    session,
                    request,
                    operation,
                ),
                "operator: move resolve dest",
            );
            return;
        };

        let mut src_paths = Vec::new();
        for src_id in &sources {
            let Some(path) = self.registry.resolve(*src_id) else {
                send_or_warn(
                    &self.events,
                    Event::from_operation_error(
                        CoreError::invalid_input(format!("Cannot resolve source {src_id:?}")),
                        session,
                        request,
                        operation,
                    ),
                    "operator: move resolve src",
                );
                return;
            };
            src_paths.push(path);
        }

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
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

                match cx.race(fs.scheme(), fs.rename(&src_path, &dst_file, &cx)).await {
                    Ok(()) => {
                        invalidate_parent_cache(&cache, &src_path);
                        invalidate_parent_cache(&cache, &dst_file);
                        invalidate_subtree_cache(&cache, &src_path);
                        affected.push(registry.clone().register(dst_file));
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
                        affected.push(registry.clone().register(dst_file));
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
                operation_complete_event(
                    &registry,
                    OperationKind::Move,
                    operation,
                    affected,
                    session,
                    completion,
                ),
                "operator: move complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn delete(
        &self,
        targets: Vec<NodeId>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        completion: CompletionShape,
    ) {
        let mut paths: Vec<(NodeId, PathBuf)> = Vec::new();
        for target in &targets {
            let Some(path) = self.registry.resolve(*target) else {
                send_or_warn(
                    &self.events,
                    Event::from_operation_error(
                        CoreError::invalid_input(format!("Cannot resolve node {target:?}")),
                        session,
                        request,
                        operation,
                    ),
                    "operator: delete resolve",
                );
                return;
            };
            paths.push((*target, path));
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

        tokio::spawn(async move {
            let cx = operation_cx(&cancel, deadline);
            let mut affected = Vec::new();
            let mut items_done = 0usize;

            for (id, path) in paths {
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
                        affected.push(id);
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
                                        Some(ProgressTarget::Node(id)),
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
                operation_complete_event(
                    &registry,
                    OperationKind::Delete,
                    operation,
                    affected,
                    session,
                    completion,
                ),
                "operator: delete complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn rename(
        &self,
        source: NodeId,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        completion: CompletionShape,
    ) {
        let Some(src_path) = self.registry.resolve(source) else {
            send_or_warn(
                &self.events,
                Event::from_operation_error(
                    CoreError::invalid_input(format!("Cannot resolve node {source:?}")),
                    session,
                    request,
                    operation,
                ),
                "operator: rename resolve",
            );
            return;
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

        tokio::spawn(async move {
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
            let id = registry.clone().register(new_path);
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
                    Some(ProgressTarget::Node(id)),
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                operation_complete_event(
                    &registry,
                    OperationKind::Rename,
                    operation,
                    vec![id],
                    session,
                    completion,
                ),
                "operator: rename complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn create_file(
        &self,
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        completion: CompletionShape,
    ) {
        let Some(path) = self.registry.resolve(parent) else {
            send_or_warn(
                &self.events,
                Event::from_operation_error(
                    CoreError::invalid_input(format!("Cannot resolve node {parent:?}")),
                    session,
                    request,
                    operation,
                ),
                "operator: create_file resolve",
            );
            return;
        };

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
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
            if let Err(e) = cx
                .race(fs.scheme(), fs.write(&full_path, &[], &cx))
                .await
            {
                send_or_warn_async(
                    &events,
                    operation_error(e, session, request, operation),
                    "operator: create_file write",
                )
                .await;
                return;
            }
            invalidate_parent_cache(&cache, &full_path);
            let id = registry.clone().register(full_path);
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
                    Some(ProgressTarget::Node(id)),
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                operation_complete_event(
                    &registry,
                    OperationKind::CreateFile,
                    operation,
                    vec![id],
                    session,
                    completion,
                ),
                "operator: create_file complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn create_folder(
        &self,
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
        completion: CompletionShape,
    ) {
        let Some(path) = self.registry.resolve(parent) else {
            send_or_warn(
                &self.events,
                Event::from_operation_error(
                    CoreError::invalid_input(format!("Cannot resolve node {parent:?}")),
                    session,
                    request,
                    operation,
                ),
                "operator: create_folder resolve",
            );
            return;
        };

        let cancel = self.arm_operation(session, operation);
        let deadline = self.default_timeout.map(|t| Instant::now() + t);
        let active = self.active_ops.clone();
        let active_operation_ids = self.active_operation_ids.clone();
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
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
            let id = registry.clone().register(full_path);
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
                    Some(ProgressTarget::Node(id)),
                ),
            )
            .await;
            send_or_warn_async(
                &events,
                operation_complete_event(
                    &registry,
                    OperationKind::CreateFolder,
                    operation,
                    vec![id],
                    session,
                    completion,
                ),
                "operator: create_folder complete",
            )
            .await;
            active.remove_if_current(session, &cancel).await;
            remove_operation_if_current(active_operation_ids, session, operation).await;
        });
    }

    fn resolve_location_node(
        &self,
        location: &LocationRef,
        kind: OperationKind,
    ) -> Result<NodeId, CoreError> {
        let capability = operation_capability_for_location(
            location,
            kind.clone(),
            &self.registry,
            self.provider.capabilities(),
        )?;
        if !capability.supported {
            let provider = capability
                .location
                .descriptor()
                .map(|descriptor| descriptor.provider().clone())
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "Resolved capability location is missing its provider descriptor",
                    )
                })?;
            let missing = capability
                .unsupported
                .clone()
                .unwrap_or(LocationCapabilityError::OperationUnsupported(kind));
            return Err(CoreError::provider_capability(
                provider,
                capability.location,
                missing,
            ));
        }
        let location = self.registry.resolve_location_ref(location)?;
        self.registry.register_location_node(location)
    }

    fn resolve_location_nodes(
        &self,
        locations: &[LocationRef],
        kind: OperationKind,
    ) -> Result<Vec<NodeId>, CoreError> {
        locations
            .iter()
            .map(|location| self.resolve_location_node(location, kind.clone()))
            .collect()
    }
}

async fn copy_dir_recursive(
    fs: &Arc<dyn FsProvider>,
    src: &Path,
    dst: &Path,
    cx: &ProviderCx<'_>,
    events: &Sender<Event>,
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
            cx.race(fs.scheme(), fs.copy(&src_child, &dst_child, cx)).await?;
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
    affected: Vec<NodeId>,
    session: SessionId,
    completion: CompletionShape,
) -> Event {
    match completion {
        CompletionShape::Node => Event::OperationCompleteCompat {
            operation_id: operation,
            operation: kind,
            success: true,
            affected,
            session,
        },
        CompletionShape::Location => Event::OperationComplete {
            operation_id: operation,
            operation: kind,
            success: true,
            affected: affected
                .into_iter()
                .filter_map(|node| registry.resolve_node_location(node))
                .collect(),
            session,
        },
    }
}

async fn emit_operation_progress(
    events: &Sender<Event>,
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
                    session,
                    request,
                    operation,
                }) => self.copy(
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                    CompletionShape::Node,
                ),
                Ok(OpsCommand::CopyLocation {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                }) => {
                    let resolved = self
                        .resolve_location_nodes(&sources, OperationKind::Copy)
                        .and_then(|sources| {
                            self.resolve_location_node(&destination, OperationKind::Copy)
                                .map(|destination| (sources, destination))
                        });
                    match resolved {
                        Ok((sources, destination)) => self.copy(
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                            CompletionShape::Location,
                        ),
                        Err(error) => send_or_warn(
                            &self.events,
                            operation_error(error, session, request, operation),
                            "operator: copy location resolve",
                        ),
                    }
                }
                Ok(OpsCommand::Move {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                }) => self.moves(
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                    CompletionShape::Node,
                ),
                Ok(OpsCommand::MoveLocation {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                }) => {
                    let resolved = self
                        .resolve_location_nodes(&sources, OperationKind::Move)
                        .and_then(|sources| {
                            self.resolve_location_node(&destination, OperationKind::Move)
                                .map(|destination| (sources, destination))
                        });
                    match resolved {
                        Ok((sources, destination)) => self.moves(
                            sources,
                            destination,
                            session,
                            request,
                            operation,
                            CompletionShape::Location,
                        ),
                        Err(error) => send_or_warn(
                            &self.events,
                            operation_error(error, session, request, operation),
                            "operator: move location resolve",
                        ),
                    }
                }
                Ok(OpsCommand::Delete {
                    targets,
                    trash,
                    session,
                    request,
                    operation,
                }) => self.delete(
                    targets,
                    trash,
                    session,
                    request,
                    operation,
                    CompletionShape::Node,
                ),
                Ok(OpsCommand::DeleteLocation {
                    targets,
                    trash,
                    session,
                    request,
                    operation,
                }) => match self.resolve_location_nodes(&targets, OperationKind::Delete) {
                    Ok(targets) => self.delete(
                        targets,
                        trash,
                        session,
                        request,
                        operation,
                        CompletionShape::Location,
                    ),
                    Err(error) => send_or_warn(
                        &self.events,
                        operation_error(error, session, request, operation),
                        "operator: delete location resolve",
                    ),
                },
                Ok(OpsCommand::Rename {
                    source,
                    new_name,
                    session,
                    request,
                    operation,
                }) => self.rename(
                    source,
                    new_name,
                    session,
                    request,
                    operation,
                    CompletionShape::Node,
                ),
                Ok(OpsCommand::RenameLocation {
                    source,
                    new_name,
                    session,
                    request,
                    operation,
                }) => match self.resolve_location_node(&source, OperationKind::Rename) {
                    Ok(source) => self.rename(
                        source,
                        new_name,
                        session,
                        request,
                        operation,
                        CompletionShape::Location,
                    ),
                    Err(error) => send_or_warn(
                        &self.events,
                        operation_error(error, session, request, operation),
                        "operator: rename location resolve",
                    ),
                },
                Ok(OpsCommand::CreateFile {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                }) => self.create_file(
                    parent,
                    name,
                    session,
                    request,
                    operation,
                    CompletionShape::Node,
                ),
                Ok(OpsCommand::CreateFileLocation {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                }) => match self.resolve_location_node(&parent, OperationKind::CreateFile) {
                    Ok(parent) => self.create_file(
                        parent,
                        name,
                        session,
                        request,
                        operation,
                        CompletionShape::Location,
                    ),
                    Err(error) => send_or_warn(
                        &self.events,
                        operation_error(error, session, request, operation),
                        "operator: create file location resolve",
                    ),
                },
                Ok(OpsCommand::CreateFolder {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                }) => self.create_folder(
                    parent,
                    name,
                    session,
                    request,
                    operation,
                    CompletionShape::Node,
                ),
                Ok(OpsCommand::CreateFolderLocation {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                }) => match self.resolve_location_node(&parent, OperationKind::CreateFolder) {
                    Ok(parent) => self.create_folder(
                        parent,
                        name,
                        session,
                        request,
                        operation,
                        CompletionShape::Location,
                    ),
                    Err(error) => send_or_warn(
                        &self.events,
                        operation_error(error, session, request, operation),
                        "operator: create folder location resolve",
                    ),
                },
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}
