//! # Transfer operations
//!
//! This module runs copy and move work outside the actor dispatch loop. Both
//! handlers use the shared provider context, event, and cache lifecycle.
//!
//! ```
//! use filer_core::model::session::SessionId;
//! use filer_core::modules::operations::operator::{OperationEventMode, OpsCommand};
//! use filer_core::{Location, LocationRef, OperationId, RequestId};
//!
//! let source = LocationRef::from_location(&Location::local("/tmp/source"));
//! let destination = LocationRef::from_location(&Location::local("/tmp/destination"));
//! let command = OpsCommand::Copy {
//!     sources: vec![source],
//!     destination,
//!     event_mode: OperationEventMode::Location,
//!     session: SessionId(1),
//!     request: RequestId(2),
//!     operation: OperationId(3),
//! };
//! assert!(matches!(command, OpsCommand::Copy { .. }));
//! ```

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::location::{Location, LocationRef};
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::{CoreError, ErrorCode, FsProvider, ProviderCx};

use super::command::OperationEventMode;
use super::operator::Operator;
use super::support::{
    emit_operation_progress, invalidate_parent_cache, invalidate_subtree_cache, is_cross_device,
    operation_complete_event, operation_cx, operation_error, remove_operation_if_current,
};
use super::target::{affected_location, resolve_direct_target, resolve_direct_targets};

impl Operator {
    pub(super) fn copy(
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
            if let Err(error) =
                super::preflight::check_transfer(fs.as_ref(), &src_paths, &dst_path, &cx).await
            {
                send_or_warn_async(
                    &events,
                    operation_error(error, session, request, operation),
                    "operator: copy preflight",
                )
                .await;
                active.remove_if_current(session, &cancel).await;
                remove_operation_if_current(active_operation_ids, session, operation).await;
                return;
            }
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
                        &ProgressScope::operation(OperationKind::Copy, session, request, operation),
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

    pub(super) fn moves(
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
            if let Err(error) =
                super::preflight::check_transfer(fs.as_ref(), &src_paths, &dst_path, &cx).await
            {
                send_or_warn_async(
                    &events,
                    operation_error(error, session, request, operation),
                    "operator: move preflight",
                )
                .await;
                active.remove_if_current(session, &cancel).await;
                remove_operation_if_current(active_operation_ids, session, operation).await;
                return;
            }
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
}

async fn copy_dir_recursive(
    fs: &Arc<dyn FsProvider>,
    src: &Path,
    dst: &Path,
    cx: &ProviderCx<'_>,
    events: &EventSink,
    scope: &ProgressScope,
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
                fs, &src_child, &dst_child, cx, events, scope, items_done,
            ))
            .await?;
        } else {
            cx.race(fs.scheme(), fs.copy(&src_child, &dst_child, cx))
                .await?;
            *items_done += 1;
            let location = LocationRef::from_location(&Location::local(dst_child));
            send_or_warn_async(
                events,
                Event::ProgressUpdated {
                    scope: scope.clone(),
                    snapshot: ProgressSnapshot::new(
                        ProgressStatus::Running,
                        ProgressPhase::Processing,
                        ProgressUnit::Item,
                        *items_done,
                        None,
                        Some(ProgressTarget::Location(location)),
                    ),
                },
                "operator: copy dir progress",
            )
            .await;
        }
    }
    Ok(())
}
