//! # Provider call context
//!
//! Carries an optional deadline and cancellation signal into provider calls.
//! Without it, a cancellation token is only polled between awaits, so a stalled
//! provider read survives its cancel. [`ProviderCx::race`] drives the provider
//! future against both the deadline and the signal, dropping the future the
//! moment either fires.
//!
//! Call sites that have no deadline or signal yet use [`ProviderCx::none`],
//! which races against nothing and returns the future's own result.

use std::future::{Future, pending};
use std::time::{Duration, Instant};

use crate::errors::CoreError;
use crate::model::cancel::CancelSignal;

/// Deadline and cancellation context threaded into a provider call.
#[derive(Clone, Copy, Default)]
pub struct ProviderCx<'a> {
    pub deadline: Option<Instant>,
    pub cancel: Option<&'a CancelSignal>,
}

impl<'a> ProviderCx<'a> {
    /// A context that enforces neither a deadline nor cancellation.
    pub const fn none() -> Self {
        Self {
            deadline: None,
            cancel: None,
        }
    }

    /// A context that observes `cancel` with no deadline.
    pub const fn with_cancel(cancel: &'a CancelSignal) -> Self {
        Self {
            deadline: None,
            cancel: Some(cancel),
        }
    }

    /// Set an absolute deadline.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Set a deadline `timeout` from now.
    pub fn with_timeout(self, timeout: Duration) -> Self {
        self.with_deadline(Instant::now() + timeout)
    }

    /// Return whether the caller's cancellation signal has fired.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_some_and(CancelSignal::is_cancelled)
    }

    /// Time left before the deadline, or `None` when no deadline is set.
    pub fn remaining(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }

    /// Run `fut` until it finishes, the deadline elapses, or `cancel` fires.
    ///
    /// A breached deadline returns [`CoreError::provider_timed_out`] tagged with
    /// `scheme`; a fired signal returns [`CoreError::cancelled`]. Both drop the
    /// in-flight future, which is what makes cancellation reach provider I/O.
    pub async fn race<F, T>(&self, scheme: &str, fut: F) -> Result<T, CoreError>
    where
        F: Future<Output = Result<T, CoreError>>,
    {
        if self.is_cancelled() {
            return Err(CoreError::cancelled());
        }

        let deadline = self.deadline;
        let cancel = self.cancel;
        tokio::pin!(fut);

        let on_deadline = async {
            match deadline {
                Some(d) => tokio::time::sleep_until(tokio::time::Instant::from_std(d)).await,
                None => pending::<()>().await,
            }
        };
        let on_cancel = async {
            match cancel {
                Some(c) => c.cancelled().await,
                None => pending::<()>().await,
            }
        };
        tokio::pin!(on_deadline);
        tokio::pin!(on_cancel);

        tokio::select! {
            result = &mut fut => result,
            _ = &mut on_deadline => {
                Err(CoreError::provider_timed_out(scheme, format!("Provider '{scheme}' timed out")))
            }
            _ = &mut on_cancel => Err(CoreError::cancelled()),
        }
    }
}
