//! # Bounded event delivery
//!
//! `EventSink` is the core-to-client event boundary. Lossless events use a
//! bounded queue and wait for the client to make room. Intermediate progress
//! updates share a bounded set of per-scope slots, so a slow client cannot turn
//! many concurrent scans into unbounded memory growth.
//!
//! ```ignore
//! let (events, receiver) = EventSink::for_runtime(work_tracker);
//! events.send(Event::SessionCreated(session))?;
//! let next = receiver.recv_async().await?;
//! # let _ = next;
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};
use tokio::sync::Notify;

use crate::actors::WorkTracker;
use crate::actors::cancel::CancellationToken;
use crate::api::events::Event;
use crate::model::progress::{ProgressScope, ProgressStatus};

/// The bounded public event queue capacity.
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSendError {
    Closed,
}

impl std::fmt::Display for EventSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("event sink is closed"),
        }
    }
}

impl std::error::Error for EventSendError {}

struct HubState {
    closed: bool,
    progress: HashMap<ProgressScope, Event>,
    progress_order: VecDeque<ProgressScope>,
    progress_capacity: usize,
    terminal_scopes: HashSet<ProgressScope>,
    terminal_order: VecDeque<ProgressScope>,
    terminal_capacity: usize,
}

struct EventHub {
    output: Sender<Event>,
    lossless_rx: Mutex<Option<Receiver<Event>>>,
    close_token: CancellationToken,
    notify: Notify,
    state: Mutex<HubState>,
}

struct HubCloseGuard(Arc<EventHub>);

impl Drop for HubCloseGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Cloneable event sender used by core actors.
#[derive(Clone)]
pub struct EventSink {
    inner: EventSinkInner,
}

impl std::fmt::Debug for EventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSink").finish_non_exhaustive()
    }
}

#[derive(Clone)]
enum EventSinkInner {
    Direct(Sender<Event>),
    Hub {
        lossless: Sender<Event>,
        hub: Arc<EventHub>,
        token: CancellationToken,
    },
}

impl EventSink {
    /// Build the production sink and its bounded client receiver.
    pub(crate) fn for_runtime(work: WorkTracker) -> (Self, Receiver<Event>) {
        Self::for_runtime_with_capacity(work, DEFAULT_EVENT_CHANNEL_CAPACITY)
    }

    pub(crate) fn for_runtime_with_capacity(
        work: WorkTracker,
        capacity: usize,
    ) -> (Self, Receiver<Event>) {
        let (output, receiver) = flume::bounded(capacity);
        let (lossless, lossless_rx) = flume::bounded(capacity);
        let state_capacity = capacity.max(1);
        let token = CancellationToken::new();
        let hub = Arc::new(EventHub {
            output,
            lossless_rx: Mutex::new(Some(lossless_rx)),
            close_token: token.clone(),
            notify: Notify::new(),
            state: Mutex::new(HubState {
                closed: false,
                progress: HashMap::new(),
                progress_order: VecDeque::new(),
                progress_capacity: state_capacity,
                terminal_scopes: HashSet::new(),
                terminal_order: VecDeque::new(),
                terminal_capacity: state_capacity,
            }),
        });
        let run_token = token.clone();
        let run_hub = hub.clone();
        let _ = work.spawn(token.clone(), async move {
            run_hub.run(run_token).await;
        });
        (
            Self {
                inner: EventSinkInner::Hub {
                    lossless,
                    hub,
                    token,
                },
            },
            receiver,
        )
    }

    pub fn send(&self, event: Event) -> Result<(), EventSendError> {
        match &self.inner {
            EventSinkInner::Direct(sender) => {
                sender.send(event).map_err(|_| EventSendError::Closed)
            }
            EventSinkInner::Hub {
                lossless,
                hub,
                token,
            } => {
                if token.is_cancelled() {
                    return Err(EventSendError::Closed);
                }
                if hub.is_closed() {
                    return Err(EventSendError::Closed);
                }
                if let Some((scope, terminal)) = progress_kind(&event) {
                    if terminal {
                        hub.mark_terminal(&scope)?;
                        lossless.send(event).map_err(|_| EventSendError::Closed)
                    } else {
                        hub.store_progress(scope, event)
                    }
                } else if hub.is_closed() {
                    Err(EventSendError::Closed)
                } else {
                    lossless.send(event).map_err(|_| EventSendError::Closed)
                }
            }
        }
    }

    pub async fn send_async(&self, event: Event) -> Result<(), EventSendError> {
        match &self.inner {
            EventSinkInner::Direct(sender) => sender
                .send_async(event)
                .await
                .map_err(|_| EventSendError::Closed),
            EventSinkInner::Hub {
                lossless,
                hub,
                token,
            } => {
                if token.is_cancelled() {
                    return Err(EventSendError::Closed);
                }
                if hub.is_closed() {
                    return Err(EventSendError::Closed);
                }
                if let Some((scope, terminal)) = progress_kind(&event) {
                    if terminal {
                        hub.mark_terminal(&scope)?;
                        tokio::select! {
                            result = lossless.send_async(event) => {
                                result.map_err(|_| EventSendError::Closed)
                            }
                            _ = token.cancelled() => Err(EventSendError::Closed),
                        }
                    } else {
                        hub.store_progress(scope, event)
                    }
                } else if hub.is_closed() {
                    Err(EventSendError::Closed)
                } else {
                    tokio::select! {
                        result = lossless.send_async(event) => {
                            result.map_err(|_| EventSendError::Closed)
                        }
                        _ = token.cancelled() => Err(EventSendError::Closed),
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn buffered_state_counts(&self) -> (usize, usize) {
        match &self.inner {
            EventSinkInner::Direct(_) => (0, 0),
            EventSinkInner::Hub { hub, .. } => {
                let state = hub
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                (state.progress.len(), state.terminal_scopes.len())
            }
        }
    }
}

impl From<Sender<Event>> for EventSink {
    fn from(sender: Sender<Event>) -> Self {
        Self {
            inner: EventSinkInner::Direct(sender),
        }
    }
}

fn progress_kind(event: &Event) -> Option<(ProgressScope, bool)> {
    let Event::ProgressUpdated { scope, snapshot } = event else {
        return None;
    };
    let terminal = matches!(
        snapshot.status,
        ProgressStatus::Completed | ProgressStatus::Cancelled | ProgressStatus::Failed
    );
    Some((scope.clone(), terminal))
}

impl EventHub {
    fn is_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .closed
    }

    fn mark_terminal(&self, scope: &ProgressScope) -> Result<(), EventSendError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.closed {
            return Err(EventSendError::Closed);
        }
        state.progress.remove(scope);
        state.progress_order.retain(|pending| pending != scope);
        if state.terminal_scopes.insert(scope.clone()) {
            state.terminal_order.push_back(scope.clone());
        }
        while state.terminal_order.len() > state.terminal_capacity {
            if let Some(expired) = state.terminal_order.pop_front() {
                state.terminal_scopes.remove(&expired);
            }
        }
        Ok(())
    }

    fn store_progress(&self, scope: ProgressScope, event: Event) -> Result<(), EventSendError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.closed {
            return Err(EventSendError::Closed);
        }
        if state.terminal_scopes.contains(&scope) {
            return Ok(());
        }
        if state.progress.contains_key(&scope) {
            state.progress.insert(scope, event);
        } else {
            while state.progress.len() >= state.progress_capacity {
                let Some(expired) = state.progress_order.pop_front() else {
                    break;
                };
                state.progress.remove(&expired);
            }
            state.progress_order.push_back(scope.clone());
            state.progress.insert(scope, event);
        }
        drop(state);
        self.notify.notify_one();
        Ok(())
    }

    async fn run(self: Arc<Self>, token: CancellationToken) {
        let _close_guard = HubCloseGuard(self.clone());
        let lossless_rx = self
            .lossless_rx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let Some(lossless_rx) = lossless_rx else {
            return;
        };
        loop {
            if self.flush_progress(&token).await.is_err() {
                break;
            }

            tokio::select! {
                _ = token.cancelled() => break,
                result = lossless_rx.recv_async() => {
                    match result {
                        Ok(event) => {
                            if self.forward(event, &token).await.is_err() {
                                self.close();
                                break;
                            }
                        }
                        Err(_) => {
                            self.close();
                            break;
                        }
                    }
                }
                _ = self.notify.notified() => {}
            }
        }
        self.close();
    }

    async fn flush_progress(&self, token: &CancellationToken) -> Result<(), EventSendError> {
        let pending = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                let Some(scope) = state.progress_order.pop_front() else {
                    return Ok(());
                };
                if let Some(pending) = state.progress.remove(&scope) {
                    break Some(pending);
                }
            }
        };

        let Some(event) = pending else {
            return Ok(());
        };
        self.forward(event, token).await
    }

    async fn forward(&self, event: Event, token: &CancellationToken) -> Result<(), EventSendError> {
        tokio::select! {
            result = self.output.send_async(event) => {
                result.map_err(|_| EventSendError::Closed)
            }
            _ = token.cancelled() => Err(EventSendError::Closed),
        }
    }

    fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        state.progress.clear();
        state.progress_order.clear();
        state.terminal_scopes.clear();
        state.terminal_order.clear();
        self.close_token.cancel();
    }
}
