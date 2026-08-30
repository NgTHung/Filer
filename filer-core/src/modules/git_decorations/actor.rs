use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;

use crate::actors::cancel::CancelMap;
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::fs_change::FsChangeKind;
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::git_decorations::backend::{GitRepository, GitStatusBackend, GitStatusResult};
use crate::modules::git_decorations::{
    FileDecorationInvalidation, GitDecorationTarget, MAX_VISIBLE_DECORATIONS,
};
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::watch::{FsChange, WatchHandle, WatchProvider};

/// Commands consumed by the Git decoration actor.
#[derive(Debug, Clone)]
pub(crate) enum GitDecorationCommand {
    Status {
        parent: LocationRef,
        visible: Vec<LocationRef>,
        session: SessionId,
        request: RequestId,
    },
    SessionDestroyed(SessionId),
}

struct WorkerResult {
    session: SessionId,
    request: RequestId,
    targets: Vec<GitDecorationTarget>,
    result: Result<GitStatusResult, CoreError>,
}

struct Subscription {
    request: RequestId,
    targets: Vec<GitDecorationTarget>,
    repository: GitRepository,
    watched_paths: Vec<PathBuf>,
}

struct WatchRegistration {
    sessions: HashSet<SessionId>,
    _handle: Box<dyn WatchHandle>,
}

/// Actor that runs Git status work away from directory loading.
pub(crate) struct GitDecorationsActor {
    commands: Receiver<GitDecorationCommand>,
    events: EventSink,
    registry: NodeRegistry,
    backend: Arc<dyn GitStatusBackend>,
    watch_provider: Arc<dyn WatchProvider>,
    change_rx: Receiver<FsChange>,
    change_tx: Sender<FsChange>,
    results: Receiver<WorkerResult>,
    result_tx: Sender<WorkerResult>,
    active: CancelMap,
    latest: HashMap<SessionId, RequestId, RandomState>,
    subscriptions: HashMap<SessionId, Subscription, RandomState>,
    watches: HashMap<PathBuf, WatchRegistration, RandomState>,
    work: WorkTracker,
}

impl GitDecorationsActor {
    pub(crate) fn new(
        commands: Receiver<GitDecorationCommand>,
        events: EventSink,
        registry: NodeRegistry,
        backend: Arc<dyn GitStatusBackend>,
        watch_provider: Arc<dyn WatchProvider>,
    ) -> Self {
        let (change_tx, change_rx) = flume::bounded(crate::DEFAULT_EVENT_CHANNEL_CAPACITY);
        let (result_tx, results) = flume::unbounded();
        Self {
            commands,
            events,
            registry,
            backend,
            watch_provider,
            change_rx,
            change_tx,
            results,
            result_tx,
            active: CancelMap::new(),
            latest: HashMap::with_hasher(RandomState::new()),
            subscriptions: HashMap::with_hasher(RandomState::new()),
            watches: HashMap::with_hasher(RandomState::new()),
            work: WorkTracker::new(),
        }
    }

    pub(crate) fn with_work_tracker(mut self, work: WorkTracker) -> Self {
        self.work = work;
        self
    }

    fn dispatch_status(
        &mut self,
        parent_ref: LocationRef,
        visible: Vec<LocationRef>,
        session: SessionId,
        request: RequestId,
    ) {
        if visible.len() > MAX_VISIBLE_DECORATIONS {
            self.emit_error(
                CoreError::invalid_input(format!(
                    "git.status accepts at most {MAX_VISIBLE_DECORATIONS} visible locations"
                )),
                session,
                request,
            );
            return;
        }
        let parent = match self.registry.resolve_location_ref(&parent_ref) {
            Ok(location) => location,
            Err(error) => {
                self.emit_error(error, session, request);
                return;
            }
        };
        let parent_path = match parent.route() {
            LocationRoute::DirectPath { path } => path,
            route => match route.require_direct_path() {
                Ok(path) => path.to_path_buf(),
                Err(error) => {
                    self.emit_error(error, session, request);
                    return;
                }
            },
        };
        let mut targets = Vec::with_capacity(visible.len());
        for location_ref in visible {
            let location = match self.registry.resolve_location_ref(&location_ref) {
                Ok(location) => location,
                Err(error) => {
                    self.emit_error(error, session, request);
                    return;
                }
            };
            let path = match location.route().require_direct_path() {
                Ok(path) => path.to_path_buf(),
                Err(error) => {
                    self.emit_error(error, session, request);
                    return;
                }
            };
            targets.push(GitDecorationTarget {
                location: LocationRef::from_location(&location),
                path,
            });
        }

        let backend = self.backend.clone();
        let result_tx = self.result_tx.clone();
        let active = self.active.clone();
        let cancel = self.active.arm(session);
        self.latest.insert(session, request);
        let worker_targets = targets.clone();
        let worker_cancel = cancel.clone();
        let spawned = self.work.spawn(cancel, async move {
            let result = backend
                .status(&parent_path, &worker_targets, &worker_cancel)
                .await;
            if !matches!(&result, Err(error) if error.code() == crate::ErrorCode::Cancelled) {
                if result_tx
                    .send_async(WorkerResult {
                        session,
                        request,
                        targets: worker_targets,
                        result,
                    })
                    .await
                    .is_err()
                {
                    tracing::debug!("Git decoration result receiver closed");
                }
            }
            active.remove_if_current(session, &worker_cancel).await;
        });
        if !spawned {
            self.emit_error(CoreError::cancelled(), session, request);
        }
    }

    fn emit_error(&self, error: CoreError, session: SessionId, request: RequestId) {
        send_or_warn(
            &self.events,
            Event::from_request_error(error, session, request),
            "git decoration error",
        );
    }

    async fn apply_result(&mut self, worker: WorkerResult) {
        if self.latest.get(&worker.session).copied() != Some(worker.request) {
            return;
        }
        let result = match worker.result {
            Ok(result) => result,
            Err(error) if error.code() == crate::ErrorCode::Cancelled => return,
            Err(error) => {
                send_or_warn_async(
                    &self.events,
                    Event::from_request_error(error, worker.session, worker.request),
                    "git decoration status error",
                )
                .await;
                return;
            }
        };

        self.remove_subscription(worker.session).await;
        if let Some(repository) = result.repository {
            let watched_paths = self.register_watches(worker.session, &repository).await;
            self.subscriptions.insert(
                worker.session,
                Subscription {
                    request: worker.request,
                    targets: worker.targets,
                    repository,
                    watched_paths,
                },
            );
        }
        send_or_warn_async(
            &self.events,
            Event::FileDecorationsUpdated {
                decorations: result.decorations,
                session: worker.session,
                request: worker.request,
            },
            "emit git decorations",
        )
        .await;
    }

    async fn register_watches(
        &mut self,
        session: SessionId,
        repository: &GitRepository,
    ) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for path in [&repository.worktree, &repository.git_dir] {
            if paths.iter().any(|registered| registered == path) {
                continue;
            }
            if let Some(watch) = self.watches.get_mut(path) {
                watch.sessions.insert(session);
                paths.push(path.clone());
                continue;
            }
            match self
                .watch_provider
                .watch(path, self.change_tx.clone())
                .await
            {
                Ok(handle) => {
                    let mut sessions = HashSet::new();
                    sessions.insert(session);
                    self.watches.insert(
                        path.clone(),
                        WatchRegistration {
                            sessions,
                            _handle: handle,
                        },
                    );
                    paths.push(path.clone());
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), error = %error, "Git decoration watch unavailable");
                }
            }
        }
        paths
    }

    async fn remove_subscription(&mut self, session: SessionId) {
        let Some(subscription) = self.subscriptions.remove(&session) else {
            return;
        };
        for path in subscription.watched_paths {
            let remove = if let Some(watch) = self.watches.get_mut(&path) {
                watch.sessions.remove(&session);
                watch.sessions.is_empty()
            } else {
                false
            };
            if remove {
                self.watches.remove(&path);
                if let Err(error) = self.watch_provider.unwatch(&path).await {
                    tracing::warn!(path = %path.display(), error = %error, "Git decoration watch cleanup failed");
                }
            }
        }
    }

    async fn dispatch_change(&self, change: FsChange) {
        let mut invalidations = Vec::new();
        for (session, subscription) in &self.subscriptions {
            let in_git_dir = change.path.starts_with(&subscription.repository.git_dir);
            let affected = if in_git_dir {
                subscription
                    .targets
                    .iter()
                    .map(|target| target.location.clone())
                    .collect::<Vec<_>>()
            } else {
                subscription
                    .targets
                    .iter()
                    .filter(|target| path_affects_target(&change, &target.path))
                    .map(|target| target.location.clone())
                    .collect::<Vec<_>>()
            };
            if !affected.is_empty() {
                invalidations.push((
                    *session,
                    subscription.request,
                    FileDecorationInvalidation {
                        locations: affected,
                    },
                ));
            }
        }
        for (session, request, invalidation) in invalidations {
            send_or_warn_async(
                &self.events,
                Event::FileDecorationsInvalidated {
                    invalidation,
                    session,
                    request,
                },
                "emit git decoration invalidation",
            )
            .await;
        }
    }

    async fn destroy_session(&mut self, session: SessionId) {
        self.active.cancel(session);
        self.latest.remove(&session);
        self.remove_subscription(session).await;
    }
}

fn path_affects_target(change: &FsChange, target: &Path) -> bool {
    change.path == target
        || change.path.starts_with(target)
        || match &change.kind {
            FsChangeKind::Renamed { from } => *from == target || from.starts_with(target),
            FsChangeKind::Created | FsChangeKind::Modified | FsChangeKind::Deleted => false,
        }
}

impl Actor for GitDecorationsActor {
    async fn run(mut self) {
        loop {
            tokio::select! {
                command = self.commands.recv_async() => {
                    match command {
                        Ok(GitDecorationCommand::Status { parent, visible, session, request }) => {
                            self.dispatch_status(parent, visible, session, request);
                        }
                        Ok(GitDecorationCommand::SessionDestroyed(session)) => {
                            self.destroy_session(session).await;
                        }
                        Err(_) => {
                            self.active.cancel_all().await;
                            break;
                        }
                    }
                }
                result = self.results.recv_async() => {
                    match result {
                        Ok(result) => self.apply_result(result).await,
                        Err(_) => break,
                    }
                }
                change = self.change_rx.recv_async() => {
                    match change {
                        Ok(change) => self.dispatch_change(change).await,
                        Err(_) => break,
                    }
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "git-decorations"
    }
}
