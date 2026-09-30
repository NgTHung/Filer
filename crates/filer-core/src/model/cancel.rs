//! # Cancellation signal
//!
//! A cheaply cloned cancellation primitive shared between long-running actors
//! and the provider layer. All clones observe the same flag.
//!
//! Polling with [`CancelSignal::is_cancelled`] drives the existing
//! between-await checks in the actors. Awaiting [`CancelSignal::cancelled`]
//! lets a caller race cancellation against an in-flight provider call, so a
//! stalled I/O future is dropped instead of outliving its cancel.
//!
//! ```
//! use filer_core::CancelSignal;
//! let signal = CancelSignal::new();
//! assert!(!signal.is_cancelled());
//! signal.cancel();
//! assert!(signal.is_cancelled());
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

/// A cancellation flag with both polling and awaiting access.
#[derive(Clone)]
pub struct CancelSignal {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancelSignal {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Flip the flag and wake every waiter blocked in [`Self::cancelled`].
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Resolve once cancelled, including when cancellation already happened.
    ///
    /// The waiter is registered before the second flag check so a `cancel`
    /// racing the registration still wakes it. `notify_waiters` keeps no
    /// permit, which is why registration must precede the final check.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }

    /// Whether two signals share the same underlying flag.
    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

impl Default for CancelSignal {
    fn default() -> Self {
        Self::new()
    }
}
