//! # Mutation queue recovery
//!
//! A failed operation retains its queue slot while the Session decides what to
//! do next. The reserved slot lets a validated retry run before its dependents
//! even when all other slots are occupied.
//!
//! ```
//! use filer_core::{OperationId, OperationQueueResolution};
//!
//! let retry = OperationQueueResolution::Retry { operation: OperationId::new() };
//! assert!(matches!(retry, OperationQueueResolution::Retry { .. }));
//! ```

use std::sync::Arc;

use flume::Sender;

use crate::model::session::SessionId;
use crate::{
    CancelSignal, CoreError, ErrorCode, ErrorTarget, Event, OperationId, OperationQueueResolution,
    RequestId,
};

use super::operator::{
    Admission, Completion, Job, Operator, SessionQueue, Update, accepted, duplicate_operation,
    worker_stopped,
};
use super::plan::{OperationPlan, OperationScope};

pub(super) struct FailedJob {
    scope: OperationScope,
    plan: Arc<OperationPlan>,
    destructive_attempted: bool,
    retry: Option<RetryValidation>,
}

struct RetryValidation {
    scope: OperationScope,
    cancel: CancelSignal,
}

impl FailedJob {
    pub fn retry_id(&mut self) -> Option<OperationId> {
        self.retry.as_ref().map(|retry| retry.scope.id)
    }
}

impl Operator {
    pub(super) async fn pause(
        &self,
        queue: &mut SessionQueue,
        scope: OperationScope,
        plan: Arc<OperationPlan>,
        destructive_attempted: bool,
    ) {
        let session = scope.session;
        let failed = scope.id;
        queue.pause(
            failed,
            FailedJob {
                scope,
                plan,
                destructive_attempted,
                retry: None,
            },
        );
        self.emit(Event::OperationQueuePaused {
            session,
            failed,
            queued: accepted_ids(queue),
        })
        .await;
    }

    pub(super) async fn emit_queue_state(
        &self,
        queue: &SessionQueue,
        session: SessionId,
        request: RequestId,
    ) {
        self.emit(Event::OperationQueueState {
            session,
            request,
            running: queue.running_id(),
            paused_on: queue.paused_id(),
            queued: accepted_ids(queue),
        })
        .await;
    }

    pub(super) async fn resolve_queue(
        &self,
        queue: &mut SessionQueue,
        session: SessionId,
        request: RequestId,
        failed: OperationId,
        resolution: OperationQueueResolution,
        updates: &Sender<Update>,
    ) {
        let operation = match resolution {
            OperationQueueResolution::Retry { operation } => Some(operation),
            _ => None,
        };
        if queue.paused_id() != Some(failed) {
            self.emit(Event::from_correlated_error(
                queue_changed(failed),
                session,
                Some(request),
                operation,
            ))
            .await;
            return;
        }
        if let Some(operation) = operation {
            self.validate_retry(queue, request, operation, updates)
                .await;
            return;
        }

        self.cancel_retry(queue, None).await;
        queue.continue_after(failed);
        if resolution == OperationQueueResolution::Cancel {
            for job in queue.drain_waiting() {
                self.cancel_waiting(job).await;
            }
        }
        self.emit_queue_state(queue, session, request).await;
    }

    async fn validate_retry(
        &self,
        queue: &mut SessionQueue,
        request: RequestId,
        operation: OperationId,
        updates: &Sender<Update>,
    ) {
        let duplicate = queue.contains(operation);
        let Some(failed) = queue.paused_work_mut() else {
            return;
        };
        let scope = OperationScope {
            session: failed.scope.session,
            request,
            id: operation,
            kind: failed.scope.kind.clone(),
        };
        if duplicate || failed.retry_id() == Some(operation) {
            self.reject(&scope, duplicate_operation()).await;
            return;
        }
        if failed.retry.is_some() {
            self.reject(&scope, queue_changed(failed.scope.id)).await;
            return;
        }
        if failed.destructive_attempted {
            self.reject(&scope, CoreError::unsupported_operation(
                "Retry cannot verify the original sources after a destructive write was attempted",
            )).await;
            return;
        }

        let cancel = CancelSignal::new();
        failed.retry = Some(RetryValidation {
            scope: scope.clone(),
            cancel: cancel.clone(),
        });
        let executor = self.executor.clone();
        let plan = failed.plan.clone();
        let completion = Completion::new(
            updates,
            Update::RetryValidated(scope.clone(), cancel.clone(), Err(worker_stopped())),
        );
        self.work
            .spawn_operation(scope.session, cancel.clone(), async move {
                let result = executor.validate_retry(&plan, &cancel).await;
                completion.send(Update::RetryValidated(scope, cancel, result));
            });
    }

    pub(super) async fn retry_validated(
        &self,
        queue: &mut SessionQueue,
        scope: OperationScope,
        cancel: &CancelSignal,
        result: Result<(), CoreError>,
    ) {
        let Some(failed) = queue.paused_work_mut().filter(|failed| {
            failed
                .retry
                .as_ref()
                .is_some_and(|retry| retry.cancel.same_instance(cancel))
        }) else {
            return;
        };
        failed.retry = None;
        if let Err(error) = result {
            self.reject(&scope, error).await;
            return;
        }
        let failed_id = failed.scope.id;
        let job = Job {
            scope: scope.clone(),
            plan: failed.plan.clone(),
            admission: Admission::Accepted,
            retry: true,
        };
        if queue.retry(failed_id, scope.id, job) {
            self.emit(accepted(&scope)).await;
        } else {
            self.reject(&scope, queue_changed(failed_id)).await;
        }
    }

    pub(super) async fn cancel_retry(&self, queue: &mut SessionQueue, id: Option<OperationId>) {
        if let Some(failed) = queue.paused_work_mut()
            && (id.is_none() || failed.retry_id() == id)
            && let Some(retry) = failed.retry.take()
        {
            retry.cancel.cancel();
            self.reject(&retry.scope, CoreError::cancelled()).await;
        }
    }
}

fn accepted_ids(queue: &SessionQueue) -> Vec<OperationId> {
    queue
        .waiting()
        .filter(|job| matches!(job.admission, Admission::Accepted))
        .map(|job| job.scope.id)
        .collect()
}

fn queue_changed(failed: OperationId) -> CoreError {
    CoreError::new(
        ErrorCode::OperationQueueStateChanged,
        Some(ErrorTarget::Operation(failed)),
        "Session mutation queue cannot apply this recovery decision",
    )
}
