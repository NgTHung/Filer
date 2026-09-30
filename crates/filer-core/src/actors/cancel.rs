use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::model::cancel::CancelSignal;
use crate::model::session::SessionId;

/// The actor-facing name for the shared [`CancelSignal`] primitive.
///
/// Tokens are armed per session by [`CancelMap`] and handed to spawned tasks.
/// The primitive lives in `model` so the provider layer can observe it without
/// depending on `actors`.
pub type CancellationToken = CancelSignal;

/// Per-session cancellation map used by long-running actors.
///
/// Wraps an `scc::HashMap<SessionId, CancellationToken>` behind an `Arc`
/// so it can be shared cheaply with spawned tasks.
///
/// # Usage pattern
///
/// ```ignore
/// // Before spawning a task:
/// let cancel = self.cancels.arm(session);
///
/// tokio::spawn(async move {
///     if cancel.is_cancelled() { return; }
///     // ... do work ...
///     if cancel.is_cancelled() { return; }
///     // ... emit result ...
///     cancels.remove_if_current(session, &cancel).await;
/// });
///
/// // To cancel from the actor loop:
/// self.cancels.cancel(session);
///
/// // On shutdown:
/// self.cancels.cancel_all().await;
/// ```
#[derive(Clone)]
pub struct CancelMap {
    inner: Arc<scc::HashMap<SessionId, CancellationToken, RandomState>>,
}

impl CancelMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    /// Cancel any in-flight task for `session` and register a fresh token.
    ///
    /// Returns the new token to pass into the spawned task.
    pub fn arm(&self, session: SessionId) -> CancellationToken {
        if let Some((_, old)) = self.inner.remove_sync(&session) {
            old.cancel();
        }
        let token = CancellationToken::new();
        let _ = self.inner.insert_sync(session, token.clone());
        token
    }

    /// Cancel the in-flight task for `session` and remove its entry.
    pub fn cancel(&self, session: SessionId) {
        if let Some((_, token)) = self.inner.remove_sync(&session) {
            token.cancel();
        }
    }

    /// Remove the entry only if it still belongs to the finishing task.
    pub async fn remove_if_current(&self, session: SessionId, token: &CancellationToken) {
        let _ = self
            .inner
            .remove_if_async(&session, |current| current.same_instance(token))
            .await;
    }

    /// Cancel all in-flight tasks — called during actor shutdown.
    pub async fn cancel_all(&self) {
        self.inner
            .iter_async(|_, v| {
                v.cancel();
                true
            })
            .await;
    }
}

impl Default for CancelMap {
    fn default() -> Self {
        Self::new()
    }
}
