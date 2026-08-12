pub mod cancel;
pub mod router;

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;
use tokio::task::{JoinHandle, JoinSet};

use crate::errors::CoreError;
use cancel::CancellationToken;

/// Trait for all actors in the system.
///
/// Actors are long-lived concurrent tasks that communicate via channels.
/// Each actor owns a `Receiver` for incoming commands and a `Sender` for
/// outgoing events. The `run()` method contains the actor's main loop.
///
/// # Implementing custom actors
///
/// Modules implement this trait for their internal actors and spawn
/// them via `ModuleContext::actors`:
///
/// ```ignore
/// struct MyActor { events: Receiver<Event> }
///
/// impl Actor for MyActor {
///     async fn run(self) {
///         while let Ok(e) = self.events.recv_async().await {
///             // react to events
///         }
///     }
///     fn name(&self) -> &'static str { "my-actor" }
/// }
/// ```
pub trait Actor: Send + 'static {
    /// Start the actor's main loop (consumes self)
    fn run(self) -> impl std::future::Future<Output = ()> + Send;

    /// Get actor name for logging
    fn name(&self) -> &'static str;
}

struct WorkState {
    closed: bool,
    shutting_down: bool,
    completed: bool,
    next_id: AtomicU64,
    tasks: JoinSet<()>,
    tokens: HashMap<u64, CancellationToken>,
    error: Option<String>,
}

struct WorkTrackerInner {
    state: Mutex<WorkState>,
    notify: Notify,
}

/// Tracks command work that outlives an actor's command loop.
///
/// Actors use this tracker instead of detaching `tokio::spawn` tasks. Shutdown
/// closes admission, cancels the work tokens, and awaits every handle before
/// returning so work owned by tracked futures cannot outlive the runtime.
#[derive(Clone)]
pub struct WorkTracker {
    inner: Arc<WorkTrackerInner>,
}

impl Default for WorkTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkTracker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WorkTrackerInner {
                state: Mutex::new(WorkState {
                    closed: false,
                    shutting_down: false,
                    completed: false,
                    next_id: AtomicU64::new(1),
                    tasks: JoinSet::new(),
                    tokens: HashMap::new(),
                    error: None,
                }),
                notify: Notify::new(),
            }),
        }
    }

    /// Spawn work unless shutdown has started.
    pub fn spawn<F>(&self, token: CancellationToken, future: F) -> bool
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while let Some(result) = state.tasks.try_join_next() {
            if let Err(join_error) = result
                && join_error.is_panic()
                && state.error.is_none()
            {
                state.error = Some(format!("tracked work panicked: {join_error}"));
            }
        }
        if state.closed {
            token.cancel();
            return false;
        }

        let id = state.next_id.fetch_add(1, Ordering::Relaxed);
        state.tokens.insert(id, token);
        let weak = Arc::downgrade(&self.inner);
        state.tasks.spawn(async move {
            future.await;
            if let Some(inner) = weak.upgrade() {
                let mut state = inner
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.tokens.remove(&id);
            }
        });
        true
    }

    pub(crate) async fn shutdown(&self) -> Result<(), CoreError> {
        loop {
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let tasks = {
                let mut state = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if state.completed {
                    return shutdown_result(state.error.as_deref());
                }
                if state.shutting_down {
                    None
                } else {
                    state.closed = true;
                    state.shutting_down = true;
                    for token in state.tokens.values() {
                        token.cancel();
                    }
                    Some((std::mem::take(&mut state.tasks), state.error.take()))
                }
            };

            let Some((mut tasks, mut error)) = tasks else {
                notified.await;
                continue;
            };

            while let Some(result) = tasks.join_next().await {
                if let Err(join_error) = result
                    && join_error.is_panic()
                    && error.is_none()
                {
                    error = Some(format!("tracked work panicked: {join_error}"));
                }
            }

            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.tokens.clear();
            state.error = error;
            state.completed = true;
            state.shutting_down = false;
            let result = shutdown_result(state.error.as_deref());
            self.inner.notify.notify_waiters();
            return result;
        }
    }

    fn abort_all(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        for token in state.tokens.values() {
            token.cancel();
        }
        state.tasks.abort_all();
    }
}

fn shutdown_result(error: Option<&str>) -> Result<(), CoreError> {
    match error {
        Some(message) => Err(CoreError::actor("work-tracker", message)),
        None => Ok(()),
    }
}

struct ActorState {
    closed: bool,
    shutting_down: bool,
    completed: bool,
    handles: Vec<JoinHandle<()>>,
    error: Option<String>,
}

struct ActorSystemInner {
    state: Mutex<ActorState>,
    notify: Notify,
}

/// Manages spawned actor tasks with uniform lifecycle control.
///
/// All actors, both built-in and custom, are spawned through `ActorSystem`.
/// It tracks their `JoinHandle`s so shutdown can stop and join them while
/// also waiting for detached command work registered with [`WorkTracker`].
pub struct ActorSystem {
    inner: Arc<ActorSystemInner>,
    work: WorkTracker,
}

impl ActorSystem {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ActorSystemInner {
                state: Mutex::new(ActorState {
                    closed: false,
                    shutting_down: false,
                    completed: false,
                    handles: Vec::new(),
                    error: None,
                }),
                notify: Notify::new(),
            }),
            work: WorkTracker::new(),
        }
    }

    /// Spawn an actor onto the tokio runtime.
    ///
    /// The actor's `run()` future executes in a new tokio task.
    /// The `JoinHandle` is tracked for shutdown.
    pub fn spawn<A: Actor>(&self, actor: A) {
        let name = actor.name();
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.handles.retain(|handle| !handle.is_finished());
        if state.closed {
            return;
        }
        let handle = tokio::spawn(async move {
            tracing::info!(actor = name, "started");
            actor.run().await;
            tracing::info!(actor = name, "stopped");
        });
        state.handles.push(handle);
    }

    /// Return the tracker used by built-in modules for detached command work.
    pub fn work_tracker(&self) -> WorkTracker {
        self.work.clone()
    }

    /// Stop actors and await all tracked command work.
    ///
    /// Aborting causes each task to drop its actor, which drops all owned
    /// channel senders/receivers. This cascades: downstream actors see
    /// their receive channels close and exit their loops naturally.
    pub async fn shutdown(&self) -> Result<(), CoreError> {
        loop {
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let handles = {
                let mut state = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if state.completed {
                    return shutdown_result(state.error.as_deref());
                }
                if state.shutting_down {
                    None
                } else {
                    state.closed = true;
                    state.shutting_down = true;
                    let handles = std::mem::take(&mut state.handles);
                    for handle in &handles {
                        handle.abort();
                    }
                    Some(handles)
                }
            };

            let Some(handles) = handles else {
                notified.await;
                continue;
            };

            let work_result = self.work.shutdown().await;
            let mut error = work_result.err().map(|error| error.to_string());
            for handle in handles {
                if let Err(join_error) = handle.await
                    && !join_error.is_cancelled()
                    && error.is_none()
                {
                    error = Some(format!("actor task failed: {join_error}"));
                }
            }

            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.error = error;
            state.completed = true;
            state.shutting_down = false;
            let result = shutdown_result(state.error.as_deref());
            self.inner.notify.notify_waiters();
            return result;
        }
    }

    /// Number of actors currently tracked.
    pub fn actor_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .handles
            .len()
    }
}

impl Drop for ActorSystem {
    fn drop(&mut self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        for handle in &state.handles {
            handle.abort();
        }
        self.work.abort_all();
    }
}
