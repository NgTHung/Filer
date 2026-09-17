//! # Mutation operations
//!
//! This module runs delete, rename, and create work outside the actor dispatch
//! loop. The handlers share cancellation, provider, event, and cache plumbing.
//!
//! ```
//! use filer_core::model::session::SessionId;
//! use filer_core::modules::operations::operator::{OperationEventMode, OpsCommand};
//! use filer_core::{Location, LocationRef, OperationId, RequestId};
//!
//! let target = LocationRef::from_location(&Location::local("/tmp/example"));
//! let command = OpsCommand::Delete {
//!     targets: vec![target],
//!     trash: false,
//!     event_mode: OperationEventMode::Location,
//!     session: SessionId(1),
//!     request: RequestId(2),
//!     operation: OperationId(3),
//! };
//! assert!(matches!(command, OpsCommand::Delete { .. }));
//! ```

use std::path::PathBuf;
use std::time::Instant;

use crate::api::events::Event;
use crate::model::location::LocationRef;
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::{CoreError, ErrorTarget};

use super::command::OperationEventMode;
use super::operator::Operator;
use super::support::{
    emit_operation_progress, invalidate_parent_cache, invalidate_subtree_cache,
    operation_complete_event, operation_cx, operation_error, remove_operation_if_current,
};
use super::target::{affected_location, resolve_direct_target};

impl Operator {
    pub(super) fn delete(
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

    pub(super) fn rename(
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

    pub(super) fn create_file(
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

    pub(super) fn create_folder(
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
