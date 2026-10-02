//! # Validated timeline
//!
//! A timeline entry is the row-free header of one event the validator has
//! already accepted. Runners derive timings only from these entries, so a
//! timestamp from a rejected or unvalidated event cannot reach a result.

use crate::schema::{Counts, Event, Output, Phase};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEntry {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub phase: Phase,
    pub action_id: Option<String>,
    pub counts: Counts,
    pub output: Option<Output>,
}

impl TimelineEntry {
    pub(super) fn from_event(event: &Event) -> Self {
        Self {
            sequence: event.sequence,
            timestamp_ns: event.timestamp_ns,
            phase: event.phase,
            action_id: event.action_id.clone(),
            counts: event.counts.clone(),
            output: event.output.clone(),
        }
    }
}
