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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use flume::{Receiver, Sender};

use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::module::CloseAck;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::{
    CancelSignal, CoreError, ErrorCode, ErrorTarget, Event, FsProvider, LocationRef, OperationId,
};

use super::DEFAULT_OPERATION_QUEUE_CAPACITY;
pub use super::command::{OperationEventMode, OpsCommand};
use super::execution::{ExecutionReport, Executor};
use super::plan::{OperationPlan, OperationScope};
use super::queue::{AdmissionError, MutationQueue};
use super::recovery::FailedJob;
pub(crate) use super::support::TrashFn;

pub struct Operator {
    commands: Receiver<OpsCommand>,
    pub(super) executor: Arc<Executor>,
    pub(super) work: WorkTracker,
}

pub(super) type SessionQueue = MutationQueue<Job, FailedJob>;

pub(super) struct Job {
    pub scope: OperationScope,
    pub plan: Arc<OperationPlan>,
    pub admission: Admission,
    pub retry: bool,
}

pub(super) enum Admission {
    Pending,
    Validating(CancelSignal),
    Accepted,
}

pub(super) enum Update {
    Validated(OperationScope, CancelSignal, Result<(), CoreError>),
    RetryValidated(OperationScope, CancelSignal, Result<(), CoreError>),
    Finished(OperationScope, Arc<OperationPlan>, ExecutionReport),
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
            events,
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
            executor: Arc::new(Executor {
                events: events.into(),
                provider,
                registry,
                trash: trash_fn,
                cache: None,
                timeout: None,
            }),
            work: WorkTracker::new(),
        }
    }

    /// Set a deadline for provider calls. A write that cannot be interrupted
    /// settles before the operation releases its queue slot.
    pub fn set_operation_timeout(&mut self, timeout: Option<Duration>) {
        Arc::make_mut(&mut self.executor).timeout = timeout;
    }

    pub fn with_cache<E: Into<EventSink>>(
        commands: Receiver<OpsCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        let mut operator = Self::new(commands, events, provider, registry);
        Arc::make_mut(&mut operator.executor).cache = Some(cache);
        operator
    }

    pub(crate) fn with_work_tracker(mut self, work: WorkTracker) -> Self {
        self.work = work;
        self
    }

    pub(super) async fn emit(&self, event: Event) {
        send_or_warn_async(&self.executor.events, event, "operator: lifecycle").await;
    }

    pub(super) async fn reject(&self, scope: &OperationScope, error: CoreError) {
        self.emit(Event::from_operation_error(
            error,
            scope.session,
            scope.request,
            scope.id,
        ))
        .await;
    }

    async fn submit(&self, command: OpsCommand, scope: OperationScope, queue: &mut SessionQueue) {
        if !self.work.accepts_new_work(scope.session) {
            self.reject(&scope, CoreError::session_closing(scope.session))
                .await;
            return;
        }
        if queue.paused_work_mut().and_then(FailedJob::retry_id) == Some(scope.id) {
            self.reject(&scope, duplicate_operation()).await;
            return;
        }
        let job = OperationPlan::resolve(
            command,
            &scope,
            &self.executor.registry,
            self.executor.provider.capabilities(),
        )
        .map(|plan| Job {
            scope: scope.clone(),
            plan: Arc::new(plan),
            admission: Admission::Pending,
            retry: false,
        });
        let admitted = job.and_then(|job| {
            queue
                .admit(scope.id, job, DEFAULT_OPERATION_QUEUE_CAPACITY)
                .map_err(|error| match error {
                    AdmissionError::Full => CoreError::new(
                        ErrorCode::OperationQueueFull,
                        Some(ErrorTarget::Session(scope.session)),
                        "Session mutation queue is full",
                    ),
                    AdmissionError::Duplicate => duplicate_operation(),
                })
        });
        if let Err(error) = admitted {
            self.reject(&scope, error).await;
        }
    }

    async fn drive(&self, session: SessionId, queue: &mut SessionQueue, updates: &Sender<Update>) {
        if !self.work.accepts_new_work(session) {
            self.stop_admission(session, queue).await;
        }
        for job in queue.waiting_mut() {
            match job.admission {
                Admission::Accepted => continue,
                Admission::Validating(_) => break,
                Admission::Pending => {}
            }
            if !self.work.accepts_new_work(session) {
                break;
            }
            if matches!(job.plan.as_ref(), OperationPlan::Transfer { .. }) {
                let cancel = CancelSignal::new();
                job.admission = Admission::Validating(cancel.clone());
                let executor = self.executor.clone();
                let scope = job.scope.clone();
                let plan = job.plan.clone();
                let update = Completion::new(
                    updates,
                    Update::Validated(scope.clone(), cancel.clone(), Err(worker_stopped())),
                );
                self.work
                    .spawn_operation(scope.session, cancel.clone(), async move {
                        let result = executor.validate_transfer(&plan, &cancel).await;
                        update.send(Update::Validated(scope, cancel, result));
                    });
                break;
            }
            job.admission = Admission::Accepted;
            self.emit(accepted(&job.scope)).await;
        }
        if queue
            .front()
            .is_some_and(|job| matches!(job.admission, Admission::Accepted))
            && let Some((_, job, cancel)) = queue.start()
        {
            let Job {
                scope, plan, retry, ..
            } = job;
            self.emit(Event::OperationStarted {
                session: scope.session,
                request: scope.request,
                operation: scope.id,
                kind: scope.kind.clone(),
            })
            .await;
            let executor = self.executor.clone();
            let update = Completion::new(
                updates,
                Update::Finished(
                    scope.clone(),
                    plan.clone(),
                    ExecutionReport {
                        outcome: Err(worker_stopped()),
                        affected: Vec::new(),
                        destructive_attempted: true,
                    },
                ),
            );
            self.work
                .spawn_operation(scope.session, cancel.clone(), async move {
                    let report = executor.run(&scope, &plan, &cancel, retry).await;
                    update.send(Update::Finished(scope, plan, report));
                });
        }
    }

    async fn update(&self, queue: &mut SessionQueue, update: Update) {
        match update {
            Update::Validated(scope, cancel, result) => {
                let Some(job) = queue.waiting_mut().find(|job| {
                    matches!(&job.admission, Admission::Validating(signal) if signal.same_instance(&cancel))
                }) else {
                    return;
                };
                let result = if self.work.accepts_new_work(scope.session) {
                    result
                } else {
                    Err(CoreError::session_closing(scope.session))
                };
                match result {
                    Ok(()) => {
                        job.admission = Admission::Accepted;
                        self.emit(accepted(&job.scope)).await;
                    }
                    Err(error) => {
                        queue.remove(scope.id);
                        self.reject(&scope, error).await;
                    }
                }
            }
            Update::RetryValidated(scope, cancel, result) => {
                self.retry_validated(queue, scope, &cancel, result).await;
            }
            Update::Finished(scope, plan, report) => {
                queue.finish();
                if !self.work.accepts_new_work(scope.session) {
                    self.stop_admission(scope.session, queue).await;
                }
                match report.outcome {
                    Ok(()) => {
                        self.emit(Event::OperationComplete {
                            operation_id: scope.id,
                            operation: scope.kind,
                            success: true,
                            affected: report.affected,
                            session: scope.session,
                        })
                        .await;
                    }
                    Err(error) => {
                        self.fail(&scope, error, report.affected).await;
                        if queue.front().is_some() {
                            self.pause(queue, scope, plan, report.destructive_attempted)
                                .await;
                        }
                    }
                }
            }
        }
    }

    async fn fail(&self, scope: &OperationScope, error: CoreError, affected: Vec<LocationRef>) {
        self.emit(Event::from_operation_failure(
            error,
            scope.session,
            scope.request,
            scope.id,
            affected,
        ))
        .await;
    }

    pub(super) async fn cancel_waiting(&self, job: Job) {
        match job.admission {
            Admission::Accepted => {
                self.fail(&job.scope, CoreError::cancelled(), Vec::new())
                    .await
            }
            Admission::Validating(cancel) => {
                cancel.cancel();
                self.reject(&job.scope, CoreError::cancelled()).await;
            }
            Admission::Pending => self.reject(&job.scope, CoreError::cancelled()).await,
        }
    }

    async fn cancel_operation(&self, queue: &mut SessionQueue, operation: OperationId) {
        if queue.cancel_running(operation) {
            return;
        }
        if let Some(job) = queue.remove(operation) {
            self.cancel_waiting(job).await;
        } else {
            self.cancel_retry(queue, Some(operation)).await;
        }
    }

    async fn stop_admission(&self, session: SessionId, queue: &mut SessionQueue) {
        let unaccepted = queue.extract_waiting(|job| !matches!(job.admission, Admission::Accepted));
        for job in unaccepted {
            if let Admission::Validating(cancel) = job.admission {
                cancel.cancel();
            }
            self.reject(&job.scope, CoreError::session_closing(session))
                .await;
        }
    }

    async fn cancel_session(&self, queue: &mut SessionQueue) {
        self.cancel_retry(queue, None).await;
        if let Some(failed) = queue.paused_id() {
            queue.continue_after(failed);
        }
        if let Some(id) = queue.running_id() {
            queue.cancel_running(id);
        }
        for job in queue.drain_waiting() {
            self.cancel_waiting(job).await;
        }
    }
}

impl Actor for Operator {
    async fn run(self) {
        let (updates, update_rx) = flume::unbounded();
        let mut queues = HashMap::<SessionId, SessionQueue>::new();
        let mut draining = HashMap::<SessionId, Vec<CloseAck>>::new();
        loop {
            let session = tokio::select! {
                command = self.commands.recv_async() => {
                    let Ok(command) = command else {
                        for queue in queues.values_mut() {
                            self.cancel_session(queue).await;
                        }
                        return;
                    };
                    match command {
                        OpsCommand::Drain(session, done) => {
                            draining.entry(session).or_default().push(done);
                            if let Some(queue) = queues.get_mut(&session) {
                                self.stop_admission(session, queue).await;
                            }
                            session
                        }
                        OpsCommand::Cancel(session) => {
                            if let Some(queue) = queues.get_mut(&session) {
                                self.cancel_session(queue).await;
                            }
                            session
                        }
                        OpsCommand::CancelOperation { session, operation } => {
                            if let Some(queue) = queues.get_mut(&session) {
                                self.cancel_operation(queue, operation).await;
                            }
                            session
                        }
                        OpsCommand::InspectQueue { session, request } => {
                            self.emit_queue_state(queues.entry(session).or_default(), session, request).await;
                            session
                        }
                        OpsCommand::ResolveQueue { session, request, failed, resolution } => {
                            self.resolve_queue(
                                queues.entry(session).or_default(), session, request, failed, resolution, &updates,
                            ).await;
                            session
                        }
                        mutation => {
                            let Some(scope) = mutation.scope() else { continue };
                            let session = scope.session;
                            self.submit(mutation, scope, queues.entry(session).or_default()).await;
                            session
                        }
                    }
                }
                Ok(update) = update_rx.recv_async() => {
                    let session = match &update {
                        Update::Validated(scope, ..) | Update::Finished(scope, ..) | Update::RetryValidated(scope, ..) => scope.session,
                    };
                    if let Some(queue) = queues.get_mut(&session) {
                        self.update(queue, update).await;
                    }
                    session
                }
            };
            if let Some(queue) = queues.get_mut(&session) {
                self.drive(session, queue, &updates).await;
                if queue.is_empty() {
                    queues.remove(&session);
                }
            }
            if !queues.contains_key(&session)
                && let Some(replies) = draining.remove(&session)
            {
                for done in replies {
                    send_or_warn(&done, Ok(()), "mutation queue drained");
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}

pub(super) fn accepted(scope: &OperationScope) -> Event {
    Event::OperationAccepted {
        session: scope.session,
        request: scope.request,
        operation: scope.id,
        kind: scope.kind.clone(),
    }
}

pub(super) fn duplicate_operation() -> CoreError {
    CoreError::invalid_input("Operation ID is already pending in this Session")
}

pub(super) fn worker_stopped() -> CoreError {
    CoreError::actor(
        "operator",
        "Operation worker stopped before reporting its outcome",
    )
}

pub(super) struct Completion {
    updates: Sender<Update>,
    fallback: Option<Update>,
}

impl Completion {
    pub(super) fn new(updates: &Sender<Update>, fallback: Update) -> Self {
        Self {
            updates: updates.clone(),
            fallback: Some(fallback),
        }
    }

    pub(super) fn send(mut self, update: Update) {
        self.fallback = None;
        self.deliver(update);
    }

    fn deliver(&self, update: Update) {
        // The receiver only disconnects when the actor stops processing its queues.
        if self.updates.send(update).is_err() {
            tracing::trace!("operator stopped before receiving worker outcome");
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        if let Some(update) = self.fallback.take() {
            self.deliver(update);
        }
    }
}
