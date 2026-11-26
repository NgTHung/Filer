//! # Mutation queue
//!
//! A Session reserves capacity until its running operation settles. A cancel
//! signal alone cannot release the slot because the provider may still be active.
//!
//! ```
//! use filer_core::modules::operations::DEFAULT_OPERATION_QUEUE_CAPACITY;
//!
//! assert!(DEFAULT_OPERATION_QUEUE_CAPACITY > 1);
//! ```

use std::collections::VecDeque;

use crate::{CancelSignal, OperationId};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    Duplicate,
    Full,
}

pub(crate) struct MutationQueue<T, F = T> {
    waiting: VecDeque<(OperationId, T)>,
    paused: Option<(OperationId, F)>,
    running: Option<(OperationId, CancelSignal)>,
}

impl<T, F> Default for MutationQueue<T, F> {
    fn default() -> Self {
        Self {
            waiting: VecDeque::new(),
            running: None,
            paused: None,
        }
    }
}

impl<T, F> MutationQueue<T, F> {
    pub(crate) fn admit(
        &mut self,
        id: OperationId,
        work: T,
        capacity: usize,
    ) -> Result<(), AdmissionError> {
        if self.contains(id) {
            return Err(AdmissionError::Duplicate);
        }
        if self.waiting.len() + usize::from(self.running.is_some() || self.paused.is_some())
            >= capacity
        {
            return Err(AdmissionError::Full);
        }
        self.waiting.push_back((id, work));
        Ok(())
    }

    pub(crate) fn front(&self) -> Option<&T> {
        self.waiting.front().map(|(_, work)| work)
    }

    pub(crate) fn waiting(&self) -> impl Iterator<Item = &T> {
        self.waiting.iter().map(|(_, work)| work)
    }

    pub(crate) fn contains(&self, id: OperationId) -> bool {
        self.running_id() == Some(id)
            || self.paused_id() == Some(id)
            || self.waiting.iter().any(|(queued, _)| *queued == id)
    }

    pub(crate) fn pause(&mut self, id: OperationId, work: F) {
        self.paused = Some((id, work));
    }

    pub(crate) fn paused_id(&self) -> Option<OperationId> {
        self.paused.as_ref().map(|(id, _)| *id)
    }

    pub(crate) fn paused_work_mut(&mut self) -> Option<&mut F> {
        self.paused.as_mut().map(|(_, work)| work)
    }

    pub(crate) fn continue_after(&mut self, failed: OperationId) -> bool {
        if self.paused_id() != Some(failed) {
            return false;
        }
        self.paused = None;
        true
    }

    pub(crate) fn retry(&mut self, failed: OperationId, id: OperationId, work: T) -> bool {
        if self.contains(id) || !self.continue_after(failed) {
            return false;
        }
        self.waiting.push_front((id, work));
        true
    }

    pub(crate) fn waiting_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.waiting.iter_mut().map(|(_, work)| work)
    }

    pub(crate) fn drain_waiting(&mut self) -> impl Iterator<Item = T> {
        self.waiting.drain(..).map(|(_, work)| work)
    }

    pub(crate) fn extract_waiting(&mut self, mut predicate: impl FnMut(&T) -> bool) -> Vec<T> {
        let mut extracted = Vec::new();
        for _ in 0..self.waiting.len() {
            if let Some((id, work)) = self.waiting.pop_front() {
                if predicate(&work) {
                    extracted.push(work);
                } else {
                    self.waiting.push_back((id, work));
                }
            }
        }
        extracted
    }

    pub(crate) fn start(&mut self) -> Option<(OperationId, T, CancelSignal)> {
        if self.running.is_some() || self.paused.is_some() {
            return None;
        }
        let (id, work) = self.waiting.pop_front()?;
        let cancel = CancelSignal::new();
        self.running = Some((id, cancel.clone()));
        Some((id, work, cancel))
    }

    pub(crate) fn running_id(&self) -> Option<OperationId> {
        self.running.as_ref().map(|(id, _)| *id)
    }

    pub(crate) fn cancel_running(&self, id: OperationId) -> bool {
        if let Some((running, token)) = &self.running
            && *running == id
        {
            token.cancel();
            return true;
        }
        false
    }

    pub(crate) fn remove(&mut self, id: OperationId) -> Option<T> {
        let index = self.waiting.iter().position(|(queued, _)| *queued == id)?;
        self.waiting.remove(index).map(|(_, work)| work)
    }

    pub(crate) fn finish(&mut self) {
        self.running = None;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.running.is_none() && self.waiting.is_empty() && self.paused.is_none()
    }
}
