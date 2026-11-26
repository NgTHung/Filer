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

pub(crate) struct MutationQueue<T> {
    waiting: VecDeque<(OperationId, T)>,
    running: Option<(OperationId, CancelSignal)>,
}

impl<T> Default for MutationQueue<T> {
    fn default() -> Self {
        Self {
            waiting: VecDeque::new(),
            running: None,
        }
    }
}

impl<T> MutationQueue<T> {
    pub(crate) fn admit(
        &mut self,
        id: OperationId,
        work: T,
        capacity: usize,
    ) -> Result<(), AdmissionError> {
        if self.running_id() == Some(id) || self.waiting.iter().any(|(queued, _)| *queued == id) {
            return Err(AdmissionError::Duplicate);
        }
        if self.waiting.len() + usize::from(self.running.is_some()) >= capacity {
            return Err(AdmissionError::Full);
        }
        self.waiting.push_back((id, work));
        Ok(())
    }

    pub(crate) fn start(&mut self) -> Option<(OperationId, T, CancelSignal)> {
        if self.running.is_some() {
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
        self.running.is_none() && self.waiting.is_empty()
    }
}
