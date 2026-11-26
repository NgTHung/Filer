//! # Bounded event delivery
//!
//! `EventSink` is the core-to-client event boundary. Lossless events use a
//! bounded queue and wait for the client to make room. Intermediate progress
//! updates share a bounded set of per-scope slots, so a slow client cannot turn
//! many concurrent scans into unbounded memory growth.
//!
//! Runtime tasks await capacity so the executor can run the event consumer.
//!
//! ```
//! use filer_core::api::event_sink::EventSink;
//! use filer_core::model::session::SessionId;
//! use filer_core::{Event, RequestId};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let (sender, receiver) = flume::bounded(1);
//! let events = EventSink::from(sender);
//! events.send_async(Event::SessionCreated {
//!     session: SessionId::new(),
//!     request: RequestId::new(),
//! }).await?;
//! let next = receiver.recv_async().await?;
//! assert!(matches!(next, Event::SessionCreated { .. }));
//! # Ok(())
//! # }
//! # tokio::runtime::Builder::new_current_thread().build()?.block_on(example())?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};
use tokio::sync::Notify;

use crate::CoreError;
#[cfg(test)]
use crate::actors::WorkTracker;
use crate::actors::cancel::CancellationToken;
use crate::api::events::Event;
use crate::model::progress::{ProgressScope, ProgressStatus};
use crate::model::session::SessionId;

/// The bounded public event queue capacity.
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EventSendError {
    #[error("event sink is closed")]
    Closed,
}

struct HubState {
    closed: bool,
    progress: HashMap<ProgressScope, Event>,
    progress_order: VecDeque<ProgressScope>,
    progress_capacity: usize,
    terminal_scopes: HashSet<ProgressScope>,
    terminal_order: VecDeque<ProgressScope>,
    terminal_capacity: usize,
}

enum Delivery {
    Event(Event),
    Finish,
}

struct EventHub {
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
        lossless: Sender<Delivery>,
        hub: Arc<EventHub>,
        token: CancellationToken,
    },
}

impl EventSink {
    pub(crate) fn runtime() -> (Self, Receiver<Event>, EventDelivery) {
        let (sink, receiver, delivery) = Self::channel(DEFAULT_EVENT_CHANNEL_CAPACITY);
        (sink, receiver, EventDelivery(tokio::spawn(delivery)))
    }

    #[cfg(test)]
    pub(crate) fn for_runtime(work: WorkTracker) -> (Self, Receiver<Event>) {
        Self::for_runtime_with_capacity(work, DEFAULT_EVENT_CHANNEL_CAPACITY)
    }

    #[cfg(test)]
    pub(crate) fn for_runtime_with_capacity(
        work: WorkTracker,
        capacity: usize,
    ) -> (Self, Receiver<Event>) {
        let (sink, receiver, delivery) = Self::channel(capacity);
        if let EventSinkInner::Hub { token, .. } = &sink.inner {
            work.spawn(token.clone(), async move {
                if let Err(error) = delivery.await {
                    tracing::trace!(%error, "Event delivery stopped");
                }
            });
        }
        (sink, receiver)
    }

    fn channel(
        capacity: usize,
    ) -> (
        Self,
        Receiver<Event>,
        impl Future<Output = Result<(), EventSendError>> + Send,
    ) {
        let (output, receiver) = flume::bounded(capacity);
        let (lossless, lossless_rx) = flume::bounded(capacity);
        let state_capacity = capacity.max(1);
        let token = CancellationToken::new();
        let hub = Arc::new(EventHub {
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
        let delivery = hub.clone().run(output, lossless_rx, token.clone());
        (
            Self {
                inner: EventSinkInner::Hub {
                    lossless,
                    hub,
                    token,
                },
            },
            receiver,
            delivery,
        )
    }

    /// Blocks the calling thread until a lossless event has queue capacity.
    ///
    /// Use [`Self::send_async`] in runtime tasks so the consumer can drain the queue.
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
                    if !terminal {
                        return hub.store_progress(scope, event);
                    }
                    hub.mark_terminal(&scope)?;
                }
                lossless
                    .send(Delivery::Event(event))
                    .map_err(|_| EventSendError::Closed)
            }
        }
    }

    /// Await lossless capacity or coalesce intermediate progress without blocking a thread.
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
                    if !terminal {
                        return hub.store_progress(scope, event);
                    }
                    hub.mark_terminal(&scope)?;
                }
                tokio::select! {
                    result = lossless.send_async(Delivery::Event(event)) => {
                        result.map_err(|_| EventSendError::Closed)
                    }
                    _ = token.cancelled() => Err(EventSendError::Closed),
                }
            }
        }
    }

    async fn finish(&self) -> Result<(), EventSendError> {
        if let EventSinkInner::Hub { hub, lossless, .. } = &self.inner {
            hub.state.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
            lossless
                .send_async(Delivery::Finish)
                .await
                .map_err(|_| EventSendError::Closed)?;
        }
        Ok(())
    }

    pub(crate) fn discard_session_progress(&self, session: SessionId) {
        if let EventSinkInner::Hub { hub, .. } = &self.inner {
            let mut state = hub.state.lock().unwrap_or_else(|e| e.into_inner());
            state.progress.retain(|scope, _| scope.session != session);
            state
                .progress_order
                .retain(|scope| scope.session != session);
            state
                .terminal_scopes
                .retain(|scope| scope.session != session);
            state
                .terminal_order
                .retain(|scope| scope.session != session);
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

pub(crate) struct EventDelivery(tokio::task::JoinHandle<Result<(), EventSendError>>);

impl EventDelivery {
    pub(crate) async fn finish(mut self, sink: &EventSink) -> Result<(), CoreError> {
        let sent = sink.finish().await;
        (&mut self.0)
            .await
            .map_err(|error| CoreError::actor("event delivery", error.to_string()))?
            .and(sent)
            .map_err(|_| CoreError::channel_closed("events"))
    }
}

impl Drop for EventDelivery {
    fn drop(&mut self) {
        self.0.abort();
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

    async fn run(
        self: Arc<Self>,
        output: Sender<Event>,
        lossless: Receiver<Delivery>,
        token: CancellationToken,
    ) -> Result<(), EventSendError> {
        let _close_guard = HubCloseGuard(self.clone());
        loop {
            self.flush_progress(&output, &token).await?;
            tokio::select! {
                _ = token.cancelled() => return Ok(()),
                result = lossless.recv_async() => match result {
                    Ok(Delivery::Event(event)) => Self::forward(&output, event, &token).await?,
                    Ok(Delivery::Finish) | Err(_) => return Ok(()),
                },
                _ = self.notify.notified() => {}
            }
        }
    }

    async fn flush_progress(
        &self,
        output: &Sender<Event>,
        token: &CancellationToken,
    ) -> Result<(), EventSendError> {
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
        Self::forward(output, event, token).await
    }

    async fn forward(
        output: &Sender<Event>,
        event: Event,
        token: &CancellationToken,
    ) -> Result<(), EventSendError> {
        tokio::select! {
            result = output.send_async(event) => {
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
