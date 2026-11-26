//! # Operation execution
//!
//! A worker owns one operation through its final provider call so the queue
//! cannot start another mutation in that Session while writes are in flight.
//!
//! ```
//! use filer_core::{CancelSignal, ProviderCx};
//!
//! let cancel = CancelSignal::new();
//! let cx = ProviderCx::with_cancel(&cancel);
//! assert!(!cx.is_cancelled());
//! ```

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::api::event_sink::EventSink;
use crate::model::registry::NodeRegistry;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::{
    CancelSignal, CoreError, ErrorTarget, Event, FsProvider, LocationRef, ProgressPhase,
    ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit, ProviderCx,
};

use super::plan::{OperationPlan, OperationScope};
use super::preflight::check_transfer;
use super::support::{TrashFn, invalidate_parent_cache, invalidate_subtree_cache, operation_cx};
use super::target::affected_location;

#[derive(Clone)]
pub(super) struct Executor {
    pub events: EventSink,
    pub provider: Arc<dyn FsProvider>,
    pub registry: NodeRegistry,
    pub trash: TrashFn,
    pub cache: Option<SharedDirCache>,
    pub timeout: Option<Duration>,
}

impl Executor {
    pub async fn validate_transfer(
        &self,
        plan: &OperationPlan,
        cancel: &CancelSignal,
    ) -> Result<(), CoreError> {
        let OperationPlan::Transfer {
            sources,
            destination,
            ..
        } = plan
        else {
            return Ok(());
        };
        let cx = operation_cx(cancel, self.timeout.map(|timeout| Instant::now() + timeout));
        check_transfer(self.provider.as_ref(), sources, destination, &cx).await
    }

    pub async fn run(
        &self,
        scope: &OperationScope,
        plan: &OperationPlan,
        cancel: &CancelSignal,
    ) -> Result<Vec<LocationRef>, CoreError> {
        let mut execution = Execution {
            executor: self,
            cx: operation_cx(cancel, self.timeout.map(|timeout| Instant::now() + timeout)),
            scope: ProgressScope::operation(
                scope.kind.clone(),
                scope.session,
                scope.request,
                scope.id,
            ),
            affected: Vec::new(),
            done: 0,
            total: None,
        };
        let result = execution.apply(plan).await;
        match &result {
            Ok(()) => {
                execution
                    .progress(
                        ProgressStatus::Completed,
                        execution.affected.last().cloned(),
                    )
                    .await
            }
            Err(error) if error.code() == crate::ErrorCode::Cancelled => {
                execution.progress(ProgressStatus::Cancelled, None).await;
            }
            Err(_) => {}
        }
        result.map(|()| execution.affected)
    }
}

pub(super) struct Execution<'a> {
    pub executor: &'a Executor,
    pub cx: ProviderCx<'a>,
    pub affected: Vec<LocationRef>,
    pub done: usize,
    scope: ProgressScope,
    total: Option<usize>,
}

impl Execution<'_> {
    async fn apply(&mut self, plan: &OperationPlan) -> Result<(), CoreError> {
        match plan {
            OperationPlan::Transfer {
                sources,
                destination,
                moving,
            } => self.transfer(sources, destination, *moving).await,
            OperationPlan::Delete { targets, trash } => self.delete(targets, *trash).await,
            OperationPlan::Rename { source, target } => self.rename(source, target).await,
            OperationPlan::Create {
                parent,
                target,
                folder,
            } => self.create(parent, target, *folder).await,
        }
    }

    pub fn provider(&self) -> &dyn FsProvider {
        self.executor.provider.as_ref()
    }

    pub async fn write<T>(
        &self,
        future: impl Future<Output = Result<T, CoreError>>,
    ) -> Result<T, CoreError> {
        let provider = self.provider();
        if provider.mutation_futures_can_be_dropped() {
            return self.cx.race(provider.scheme(), future).await;
        }
        if self.cx.is_cancelled() {
            return Err(CoreError::cancelled());
        }
        if self.cx.remaining() == Some(Duration::ZERO) {
            return Err(CoreError::provider_timed_out(
                provider.scheme(),
                "Mutation deadline elapsed",
            ));
        }
        future.await
    }

    pub fn invalidate_removed(&self, path: &Path) {
        invalidate_parent_cache(&self.executor.cache, path);
        invalidate_subtree_cache(&self.executor.cache, path);
    }

    pub fn record_target(&mut self, path: PathBuf) {
        invalidate_parent_cache(&self.executor.cache, &path);
        self.affected
            .push(affected_location(&self.executor.registry, path));
    }

    pub async fn progress(&self, status: ProgressStatus, current: Option<LocationRef>) {
        let phase = if status == ProgressStatus::Completed {
            ProgressPhase::Finalizing
        } else {
            ProgressPhase::Processing
        };
        send_or_warn_async(
            &self.executor.events,
            Event::ProgressUpdated {
                scope: self.scope.clone(),
                snapshot: ProgressSnapshot::new(
                    status,
                    phase,
                    ProgressUnit::Item,
                    self.done,
                    self.total,
                    current.map(ProgressTarget::Location),
                ),
            },
            "operator: progress",
        )
        .await;
    }

    pub fn set_total(&mut self, total: usize) {
        self.total = Some(total);
    }

    pub async fn require_missing(&self, source: &Path, target: &Path) -> Result<(), CoreError> {
        let fs = self.provider();
        if self
            .cx
            .race(fs.scheme(), fs.exists(target, &self.cx))
            .await?
        {
            return Err(CoreError::collision(
                ErrorTarget::Path(source.to_path_buf()),
                ErrorTarget::Path(target.to_path_buf()),
            ));
        }
        Ok(())
    }
}
