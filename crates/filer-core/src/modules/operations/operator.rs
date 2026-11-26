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
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::{CancelSignal, CoreError, ErrorCode, ErrorTarget, Event, FsProvider, LocationRef};

use super::DEFAULT_OPERATION_QUEUE_CAPACITY;
pub use super::command::{OperationEventMode, OpsCommand};
use super::execution::Executor;
use super::plan::{OperationPlan, OperationScope};
use super::queue::{AdmissionError, MutationQueue};
pub(crate) use super::support::TrashFn;

pub struct Operator {
    commands: Receiver<OpsCommand>,
    executor: Arc<Executor>,
    work: WorkTracker,
}

struct Job {
    scope: OperationScope,
    plan: Arc<OperationPlan>,
    admission: Admission,
}

enum Admission {
    Pending,
    Validating(CancelSignal),
    Accepted,
}

enum Update {
    Validated(OperationScope, CancelSignal, Result<(), CoreError>),
    Finished(OperationScope, Result<Vec<LocationRef>, CoreError>),
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

    async fn emit(&self, event: Event) {
        send_or_warn_async(&self.executor.events, event, "operator: lifecycle").await;
    }

    async fn reject(&self, scope: &OperationScope, error: CoreError) {
        self.emit(Event::from_operation_error(
            error,
            scope.session,
            scope.request,
            scope.id,
        ))
        .await;
    }

    async fn submit(
        &self,
        command: OpsCommand,
        scope: OperationScope,
        queue: &mut MutationQueue<Job>,
    ) {
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
                    AdmissionError::Duplicate => {
                        CoreError::invalid_input("Operation ID is already pending in this Session")
                    }
                })
        });
        if let Err(error) = admitted {
            self.reject(&scope, error).await;
        }
    }

    async fn drive(&self, queue: &mut MutationQueue<Job>, updates: &Sender<Update>) {
        for job in queue.waiting_mut() {
            match job.admission {
                Admission::Accepted => continue,
                Admission::Validating(_) => break,
                Admission::Pending => {}
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
                self.work.spawn(cancel.clone(), async move {
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
            let Job { scope, plan, .. } = job;
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
                Update::Finished(scope.clone(), Err(worker_stopped())),
            );
            self.work.spawn(cancel.clone(), async move {
                let result = executor.run(&scope, &plan, &cancel).await;
                update.send(Update::Finished(scope, result));
            });
        }
    }

    async fn update(&self, queue: &mut MutationQueue<Job>, update: Update) {
        match update {
            Update::Validated(scope, cancel, result) => {
                let Some(job) = queue.waiting_mut().find(|job| {
                    matches!(&job.admission, Admission::Validating(signal) if signal.same_instance(&cancel))
                }) else {
                    return;
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
            Update::Finished(scope, result) => {
                queue.finish();
                match result {
                    Ok(affected) => {
                        self.emit(Event::OperationComplete {
                            operation_id: scope.id,
                            operation: scope.kind,
                            success: true,
                            affected,
                            session: scope.session,
                        })
                        .await
                    }
                    Err(error) => self.reject(&scope, error).await,
                }
            }
        }
    }

    async fn cancel_waiting(&self, job: Job) {
        if let Admission::Validating(cancel) = job.admission {
            cancel.cancel();
        }
        self.reject(&job.scope, CoreError::cancelled()).await;
    }

    async fn cancel_session(&self, queue: &mut MutationQueue<Job>) {
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
        let mut queues = HashMap::<SessionId, MutationQueue<Job>>::new();
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
                        OpsCommand::Cancel(session) => {
                            if let Some(queue) = queues.get_mut(&session) {
                                self.cancel_session(queue).await;
                            }
                            session
                        }
                        OpsCommand::CancelOperation { session, operation } => {
                            if let Some(queue) = queues.get_mut(&session)
                                && !queue.cancel_running(operation)
                                && let Some(job) = queue.remove(operation)
                            {
                                self.cancel_waiting(job).await;
                            }
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
                        Update::Validated(scope, ..) | Update::Finished(scope, ..) => scope.session,
                    };
                    if let Some(queue) = queues.get_mut(&session) {
                        self.update(queue, update).await;
                    }
                    session
                }
            };
            if let Some(queue) = queues.get_mut(&session) {
                self.drive(queue, &updates).await;
                if queue.is_empty() {
                    queues.remove(&session);
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}

fn accepted(scope: &OperationScope) -> Event {
    Event::OperationAccepted {
        session: scope.session,
        request: scope.request,
        operation: scope.id,
        kind: scope.kind.clone(),
    }
}

fn worker_stopped() -> CoreError {
    CoreError::actor(
        "operator",
        "Operation worker stopped before reporting its outcome",
    )
}

struct Completion {
    updates: Sender<Update>,
    fallback: Option<Update>,
}

impl Completion {
    fn new(updates: &Sender<Update>, fallback: Update) -> Self {
        Self {
            updates: updates.clone(),
            fallback: Some(fallback),
        }
    }

    fn send(mut self, update: Update) {
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
