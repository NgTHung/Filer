//! # Adapter traces
//!
//! `AdapterTrace` records milestones while an adapter works and builds the
//! protocol events only in `finish`. Row projection, digests, and JSON
//! encoding therefore happen after the measured actions, so they cannot delay
//! a later milestone in the same sample. Sequence numbers and sample totals
//! are derived here instead of in each adapter.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::canonical::{CanonicalRow, canonical_digest};
use crate::schema::{
    Continuation, Counts, Event, Field, MetricValue, Output, OutputScope, Phase, Request, Row,
    Status,
};

pub struct AdapterTrace {
    request: Request,
    origin: Instant,
    pending: Vec<PendingEvent>,
    totals: Counts,
}

/// One committed output: its rows, cumulative action counts, and output shape.
#[derive(Clone, Debug)]
pub struct Milestone {
    pub phase: Phase,
    pub counts: Counts,
    pub rows: Vec<CanonicalRow>,
    pub scope: OutputScope,
    pub continuation: Continuation,
}

struct PendingEvent {
    timestamp_ns: u64,
    phase: Phase,
    action_id: Option<String>,
    counts: Counts,
    output: Option<PendingOutput>,
}

struct PendingOutput {
    rows: Vec<CanonicalRow>,
    scope: OutputScope,
    continuation: Continuation,
}

impl AdapterTrace {
    /// Starts the sample clock; every timestamp is relative to this instant.
    pub fn new(request: Request) -> Self {
        Self {
            request,
            origin: Instant::now(),
            pending: Vec::new(),
            totals: zero_counts(),
        }
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Nanoseconds on the adapter process's monotonic clock.
    pub fn now_ns(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    pub fn sample_started(&mut self) {
        self.push(Phase::SampleStarted, None, zero_counts(), None);
    }

    /// Call immediately before invoking the measured action.
    pub fn action_started(&mut self, action_id: &str) {
        self.push(Phase::ActionStarted, Some(action_id), zero_counts(), None);
    }

    /// Records an output milestone observed at `timestamp_ns`.
    ///
    /// Take the timestamp once the rows exist and before any later work, so
    /// it marks when a consumer could first use them.
    pub fn milestone(&mut self, timestamp_ns: u64, action_id: &str, milestone: Milestone) {
        self.pending.push(PendingEvent {
            timestamp_ns,
            phase: milestone.phase,
            action_id: Some(action_id.to_string()),
            counts: milestone.counts,
            output: Some(PendingOutput {
                rows: milestone.rows,
                scope: milestone.scope,
                continuation: milestone.continuation,
            }),
        });
    }

    pub fn action_completed(&mut self, action_id: &str, counts: Counts) {
        self.totals = add_counts(&self.totals, &counts);
        self.push(Phase::ActionCompleted, Some(action_id), counts, None);
    }

    /// Ends the sample and builds its events in sequence order.
    pub fn finish(mut self, status: Status, metrics: BTreeMap<String, MetricValue>) -> Vec<Event> {
        let completed_at = self.now_ns();
        let totals = self.totals.clone();
        let mut events = self
            .pending
            .drain(..)
            .enumerate()
            .map(|(sequence, pending)| build_event(&self.request, sequence as u64, pending))
            .collect::<Vec<_>>();
        let mut terminal = build_event(
            &self.request,
            events.len() as u64,
            PendingEvent {
                timestamp_ns: completed_at,
                phase: Phase::SampleCompleted,
                action_id: None,
                counts: totals,
                output: None,
            },
        );
        terminal.metrics = metrics;
        terminal.status = Some(status);
        events.push(terminal);
        events
    }

    fn push(
        &mut self,
        phase: Phase,
        action_id: Option<&str>,
        counts: Counts,
        output: Option<PendingOutput>,
    ) {
        let timestamp_ns = self.now_ns();
        self.pending.push(PendingEvent {
            timestamp_ns,
            phase,
            action_id: action_id.map(str::to_string),
            counts,
            output,
        });
    }
}

fn build_event(request: &Request, sequence: u64, pending: PendingEvent) -> Event {
    let (rows, output) = match pending.output {
        Some(output) => {
            let digest_fields = match output.scope {
                OutputScope::Membership => vec![Field::Identity],
                OutputScope::Metadata => Field::ALL.to_vec(),
                _ => request.requested_fields.clone(),
            };
            let digest = canonical_digest(output.scope.as_str(), &digest_fields, &output.rows);
            let rows = output
                .rows
                .iter()
                .map(|row| project(row, &request.requested_fields))
                .collect();
            let output = Output {
                scope: output.scope,
                digest,
                row_count: output.rows.len() as u64,
                continuation: output.continuation,
            };
            (rows, Some(output))
        }
        None => (Vec::new(), None),
    };
    Event {
        protocol_version: request.protocol_version,
        message_type: "run_event".to_string(),
        run_id: request.run_id.clone(),
        sample_id: request.sample_id.clone(),
        process_id: request.process_id.clone(),
        order_id: request.order_id.clone(),
        sequence,
        timestamp_ns: pending.timestamp_ns,
        phase: pending.phase,
        action_id: pending.action_id,
        counts: pending.counts,
        rows,
        output,
        metrics: BTreeMap::new(),
        status: None,
    }
}

fn project(row: &CanonicalRow, fields: &[Field]) -> Row {
    Row {
        identity: fields
            .contains(&Field::Identity)
            .then(|| row.identity.clone()),
        kind: fields.contains(&Field::Kind).then_some(row.kind),
        size_bytes: fields.contains(&Field::SizeBytes).then_some(row.size_bytes),
        modified_unix_ns: fields
            .contains(&Field::ModifiedUnixNs)
            .then_some(row.modified_unix_ns),
    }
}

fn zero_counts() -> Counts {
    Counts {
        examined: MetricValue::Observed(0),
        accepted: MetricValue::Observed(0),
        emitted: MetricValue::Observed(0),
        visible: MetricValue::Observed(0),
    }
}

fn add_counts(left: &Counts, right: &Counts) -> Counts {
    Counts {
        examined: add_metric(&left.examined, &right.examined),
        accepted: add_metric(&left.accepted, &right.accepted),
        emitted: add_metric(&left.emitted, &right.emitted),
        visible: add_metric(&left.visible, &right.visible),
    }
}

/// A total is observed only when every action observed it.
fn add_metric(left: &MetricValue, right: &MetricValue) -> MetricValue {
    match (left, right) {
        (MetricValue::Observed(left), MetricValue::Observed(right)) => {
            MetricValue::Observed(left.saturating_add(*right))
        }
        (MetricValue::Unavailable(reason), _) | (_, MetricValue::Unavailable(reason)) => {
            MetricValue::Unavailable(*reason)
        }
    }
}
