//! # Request and trace validation
//!
//! This module exposes the two benchmark seams. `RunValidator` admits request
//! identities once, while `SampleValidator` owns one event state machine until
//! explicit EOF finalization.

use std::collections::BTreeSet;

use crate::manifests::ValidatedManifest;
use crate::scenarios::{ActionKind, ScenarioPlan, validate_request};
use crate::schema::{
    Event, Filter, Phase, Request, Status, StatusKind, parse_event_line, parse_request_bytes,
};
use crate::{ErrorCode, ProtocolError};

mod output_validation;
mod trace_state;

use trace_state::{ActionState, CountsState, TraceState};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredCapabilities {
    supported_scenarios: BTreeSet<String>,
    pub streaming_unfiltered_listing: bool,
    pub examined_count_observable: bool,
}

impl DeclaredCapabilities {
    pub fn new<I, S>(
        supported_scenarios: I,
        streaming_unfiltered_listing: bool,
        examined_count_observable: bool,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            supported_scenarios: supported_scenarios.into_iter().map(Into::into).collect(),
            streaming_unfiltered_listing,
            examined_count_observable,
        }
    }

    pub fn supports(&self, scenario_id: &str) -> bool {
        self.supported_scenarios.contains(scenario_id)
    }

    pub fn supported_scenarios(&self) -> impl Iterator<Item = &str> {
        self.supported_scenarios.iter().map(String::as_str)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateResult {
    Passed,
    Failed,
    NotEvaluable,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralGates {
    pub first_page_examined: GateResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSample {
    pub request: Request,
    pub status: Status,
    pub structural_gates: StructuralGates,
    pub event_count: usize,
}

pub struct RunValidator {
    manifest: ValidatedManifest,
    capabilities: DeclaredCapabilities,
    requested_metrics: BTreeSet<String>,
    accepted_samples: BTreeSet<(String, String)>,
}

impl RunValidator {
    pub fn new(
        manifest: ValidatedManifest,
        capabilities: DeclaredCapabilities,
        requested_metrics: impl IntoIterator<Item = String>,
    ) -> Result<Self, ProtocolError> {
        let requested_metrics = requested_metrics.into_iter().collect::<BTreeSet<_>>();
        for name in &requested_metrics {
            if name.is_empty()
                || name.len() > 128
                || !name.bytes().enumerate().all(|(index, byte)| {
                    (index == 0 && byte.is_ascii_alphanumeric())
                        || (index > 0 && (byte.is_ascii_alphanumeric() || b"._:-".contains(&byte)))
                })
            {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidSchema,
                    "requested metric name is not a valid identifier",
                ));
            }
        }
        Ok(Self {
            manifest,
            capabilities,
            requested_metrics,
            accepted_samples: BTreeSet::new(),
        })
    }

    pub fn start_sample<'validator>(
        &'validator mut self,
        request_bytes: &[u8],
    ) -> Result<SampleValidator<'validator>, ProtocolError> {
        let request = parse_request_bytes(request_bytes)?;
        let sample_key = (request.run_id.clone(), request.sample_id.clone());
        if self.accepted_samples.contains(&sample_key) {
            return Err(ProtocolError::new(
                ErrorCode::DuplicateSample,
                "run_id and sample_id were already accepted",
            ));
        }
        let plan = validate_request(&request, &self.manifest)?;
        self.accepted_samples.insert(sample_key);
        let gate = if plan.first_page_gate && self.capabilities.streaming_unfiltered_listing {
            GateResult::NotEvaluable
        } else {
            GateResult::NotApplicable
        };
        Ok(SampleValidator {
            manifest: &self.manifest,
            capabilities: &self.capabilities,
            requested_metrics: &self.requested_metrics,
            request,
            plan,
            state: TraceState::new(gate),
        })
    }

    pub fn accepted_sample_count(&self) -> usize {
        self.accepted_samples.len()
    }
}

pub struct SampleValidator<'validator> {
    manifest: &'validator ValidatedManifest,
    capabilities: &'validator DeclaredCapabilities,
    requested_metrics: &'validator BTreeSet<String>,
    request: Request,
    plan: ScenarioPlan,
    state: TraceState,
}

impl SampleValidator<'_> {
    pub fn ingest_line(&mut self, line: &[u8]) -> Result<(), ProtocolError> {
        let event = parse_event_line(line).map_err(|error| error.with_line(self.state.line + 1))?;
        let action = event.action_id.clone();
        let sequence = event.sequence;
        self.state.line += 1;
        self.ingest_event(event).map_err(|error| {
            let error = error.with_sequence(sequence);
            match action {
                Some(action) => error.with_action(action),
                None => error,
            }
        })
    }

    pub fn finish(self) -> Result<ValidatedSample, ProtocolError> {
        if !self.state.started {
            return Err(ProtocolError::new(
                ErrorCode::InvalidStatus,
                "sample.started was not observed before EOF",
            ));
        }
        let status = self.state.terminal_status.ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidStatus,
                "sample.completed was not observed before EOF",
            )
        })?;
        if status.kind == StatusKind::Success
            && (self.state.current_action.is_some()
                || self.state.next_action_index != self.plan.actions.len())
        {
            return Err(ProtocolError::new(
                ErrorCode::MissingRequiredPhase,
                "success trace ended before every action completed",
            ));
        }
        Ok(ValidatedSample {
            request: self.request,
            status,
            structural_gates: StructuralGates {
                first_page_examined: self.state.gate,
            },
            event_count: self.state.events,
        })
    }

    fn ingest_event(&mut self, event: Event) -> Result<(), ProtocolError> {
        if self.state.completed {
            let code = if event.phase == Phase::SampleCompleted {
                ErrorCode::DuplicatePhase
            } else {
                ErrorCode::InvalidStatus
            };
            return Err(ProtocolError::new(
                code,
                "event arrived after sample.completed",
            ));
        }
        self.check_correlation(&event)?;
        self.check_sequence_and_clock(&event)?;
        self.state.events += 1;
        match event.phase {
            Phase::SampleStarted => self.sample_started(event),
            Phase::SampleCompleted => self.sample_completed(event),
            Phase::ActionStarted => self.action_started(event),
            Phase::ActionCompleted => self.action_completed(event),
            _ => self.action_milestone(event),
        }
    }

    fn check_correlation(&self, event: &Event) -> Result<(), ProtocolError> {
        if event.run_id != self.request.run_id
            || event.sample_id != self.request.sample_id
            || event.process_id != self.request.process_id
            || event.order_id != self.request.order_id
        {
            return Err(ProtocolError::new(
                ErrorCode::CorrelationMismatch,
                "event correlation identifiers do not match the request",
            ));
        }
        Ok(())
    }

    fn check_sequence_and_clock(&mut self, event: &Event) -> Result<(), ProtocolError> {
        if event.sequence != self.state.next_sequence {
            return Err(ProtocolError::new(
                ErrorCode::InvalidSequence,
                "event sequence must start at zero and increase by one",
            ));
        }
        if self
            .state
            .last_timestamp
            .is_some_and(|timestamp| event.timestamp_ns < timestamp)
        {
            return Err(ProtocolError::new(
                ErrorCode::ClockRegression,
                "event timestamp decreased within one sample",
            ));
        }
        self.state.next_sequence += 1;
        self.state.last_timestamp = Some(event.timestamp_ns);
        Ok(())
    }

    fn sample_started(&mut self, event: Event) -> Result<(), ProtocolError> {
        if self.state.started {
            return Err(ProtocolError::new(
                ErrorCode::DuplicatePhase,
                "sample.started may occur only once",
            ));
        }
        if event.sequence != 0 || event.action_id.is_some() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "sample.started must be the first singleton phase",
            ));
        }
        if event.status.is_some() || event.output.is_some() || !event.rows.is_empty() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidStatus,
                "sample.started cannot carry output, rows, or status",
            ));
        }
        let counts = CountsState::from_event(&event);
        if !counts.is_zero() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidCounts,
                "sample.started counts must be zero",
            ));
        }
        self.state.started = true;
        Ok(())
    }

    fn sample_completed(&mut self, event: Event) -> Result<(), ProtocolError> {
        if !self.state.started || self.state.completed || event.action_id.is_some() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidStatus,
                "sample.completed is not legal at this point",
            ));
        }
        if !event.rows.is_empty() || event.output.is_some() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidStatus,
                "sample.completed cannot carry rows or output",
            ));
        }
        let status = event.status.clone().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidStatus,
                "sample.completed requires a terminal status",
            )
        })?;
        if status.kind == StatusKind::Success
            && !self.capabilities.supports(&self.request.scenario_id)
        {
            return Err(ProtocolError::new(
                ErrorCode::UnsupportedReportedAsSuccess,
                "adapter reported success for an undeclared scenario",
            ));
        }
        self.validate_sample_metrics(&event)?;
        self.validate_sample_counts(&event)?;
        if status.kind == StatusKind::Success
            && (self.state.current_action.is_some()
                || self.state.next_action_index != self.plan.actions.len())
        {
            return Err(ProtocolError::new(
                ErrorCode::MissingRequiredPhase,
                "success trace is missing a required action",
            ));
        }
        self.state.terminal_status = Some(status);
        self.state.completed = true;
        Ok(())
    }

    fn validate_sample_metrics(&self, event: &Event) -> Result<(), ProtocolError> {
        for name in self.requested_metrics {
            if !event.metrics.contains_key(name) {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidSchema,
                    "requested metric is missing from sample.completed",
                )
                .with_field(format!("metrics.{name}")));
            }
        }
        Ok(())
    }

    fn validate_sample_counts(&self, event: &Event) -> Result<(), ProtocolError> {
        let actual = CountsState::from_event(event);
        if !self.state.totals.matches(&actual) {
            return Err(ProtocolError::new(
                ErrorCode::InvalidCounts,
                "sample phase counts do not equal completed action totals",
            ));
        }
        Ok(())
    }

    fn action_started(&mut self, event: Event) -> Result<(), ProtocolError> {
        if !self.state.started || self.state.current_action.is_some() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "action.started requires an idle started sample",
            ));
        }
        let action_id = event.action_id.as_deref().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidAction,
                "action.started requires action_id",
            )
        })?;
        let plan = self
            .plan
            .actions
            .get(self.state.next_action_index)
            .ok_or_else(|| ProtocolError::new(ErrorCode::InvalidAction, "no action remains"))?;
        if plan.id != action_id {
            return Err(ProtocolError::new(
                ErrorCode::InvalidAction,
                "action.started does not match the next scenario action",
            ));
        }
        if event.status.is_some() || event.output.is_some() || !event.rows.is_empty() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "action.started cannot carry rows, output, or status",
            ));
        }
        let counts = CountsState::from_event(&event);
        if !counts.is_zero() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidCounts,
                "action.started must reset every count to zero",
            ));
        }
        self.state.current_action = Some(ActionState::new(plan.clone()));
        self.state.next_action_index += 1;
        Ok(())
    }

    fn action_completed(&mut self, event: Event) -> Result<(), ProtocolError> {
        let action_id = event.action_id.as_deref().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidAction,
                "action.completed requires action_id",
            )
        })?;
        let action = self.state.current_action.as_ref().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidAction,
                "action.completed has no active action",
            )
        })?;
        if action.plan.id != action_id {
            return Err(ProtocolError::new(
                ErrorCode::InvalidAction,
                "action.completed does not match the active action",
            ));
        }
        if event.status.is_some() || event.output.is_some() || !event.rows.is_empty() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "action.completed cannot carry rows, output, or status",
            ));
        }
        if let Some(missing) = action
            .plan
            .required
            .iter()
            .find(|phase| !action.seen.contains(phase))
        {
            return Err(ProtocolError::new(
                ErrorCode::MissingRequiredPhase,
                format!("action is missing {}", missing.as_str()),
            ));
        }
        self.validate_counts(&event, false)?;
        let action = self.state.current_action.take().ok_or_else(|| {
            ProtocolError::new(ErrorCode::InvalidAction, "active action disappeared")
        })?;
        self.state.totals.add(&action.counts)?;
        Ok(())
    }

    fn action_milestone(&mut self, event: Event) -> Result<(), ProtocolError> {
        let action_id = event.action_id.as_deref().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidAction,
                "action milestone requires action_id",
            )
        })?;
        let action = self.state.current_action.as_ref().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::InvalidAction,
                "action milestone has no active action",
            )
        })?;
        if action.plan.id != action_id {
            return Err(ProtocolError::new(
                ErrorCode::InvalidAction,
                "event belongs to a stale or unknown action",
            ));
        }
        if !action.plan.required.contains(&event.phase)
            && !action.plan.optional.contains(&event.phase)
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "phase is not allowed for this action",
            ));
        }
        if action.seen.contains(&event.phase) {
            return Err(ProtocolError::new(
                ErrorCode::DuplicatePhase,
                "action phase was emitted more than once",
            ));
        }
        self.validate_milestone_order(action, event.phase)?;
        if event.status.is_some() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidStatus,
                "only sample.completed may carry terminal status",
            ));
        }
        self.validate_counts(&event, true)?;
        let rows = self.validate_rows(&event.rows)?;
        self.validate_milestone_output(&event, &rows)?;
        self.record_milestone(event.phase, rows)?;
        if let Some(action) = self.state.current_action.as_mut() {
            action.seen.push(event.phase);
        }
        Ok(())
    }

    fn validate_milestone_order(
        &self,
        action: &ActionState,
        phase: Phase,
    ) -> Result<(), ProtocolError> {
        if action.plan.optional.contains(&phase) && !action.plan.required.contains(&phase) {
            if action
                .seen
                .iter()
                .any(|seen| action.plan.required.contains(seen))
            {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidPhase,
                    "optional milestone must precede required action output",
                ));
            }
            return Ok(());
        }
        let position = action
            .plan
            .required
            .iter()
            .position(|required| *required == phase)
            .ok_or_else(|| {
                ProtocolError::new(
                    ErrorCode::InvalidPhase,
                    "phase has no position in the action sequence",
                )
            })?;
        let missing_prior = action.plan.required[..position].iter().any(|required| {
            if matches!(action.plan.kind, ActionKind::Open { .. })
                && matches!(
                    (phase, *required),
                    (Phase::ViewportCommitted, Phase::PageCommitted)
                        | (Phase::PageCommitted, Phase::ViewportCommitted)
                )
            {
                return false;
            }
            !action.seen.contains(required)
        });
        if missing_prior {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "action milestones are out of order",
            ));
        }
        Ok(())
    }

    fn validate_counts(&mut self, event: &Event, output_phase: bool) -> Result<(), ProtocolError> {
        let current = CountsState::from_event(event);
        let previous = self
            .state
            .current_action
            .as_ref()
            .map(|action| &action.counts);
        if let Some(previous) = previous {
            current.validate_progress(previous)?;
            let repeated_continuation_proof = event.phase == Phase::ListingCompleted
                && self.state.current_action.as_ref().is_some_and(|action| {
                    matches!(action.plan.kind, ActionKind::Page { page_number: 40 })
                });
            if repeated_continuation_proof && current != *previous {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidCounts,
                    "continuation membership proof cannot increase action counts",
                ));
            }
        }
        current.validate_relations()?;
        if output_phase {
            if current.emitted.is_none() || current.visible.is_none() {
                return Err(ProtocolError::new(
                    ErrorCode::RequiredCountUnavailable,
                    "commit phase requires emitted and visible counts",
                ));
            }
            if matches!(
                event.phase,
                Phase::RowFirst
                    | Phase::ViewportCommitted
                    | Phase::PageCommitted
                    | Phase::TransformCompleted
            ) && current
                .emitted
                .zip(event.output.as_ref().map(|output| output.row_count))
                .is_some_and(|(emitted, row_count)| emitted < row_count)
            {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidCounts,
                    "emitted count cannot be smaller than the committed rows",
                ));
            }
        }
        let filter_requires_accepted = matches!(self.request.filter, Filter::NameContains { .. })
            || self
                .state
                .current_action
                .as_ref()
                .is_some_and(|action| matches!(action.plan.kind, ActionKind::FilterName));
        if filter_requires_accepted && current.accepted.is_none() {
            return Err(ProtocolError::new(
                ErrorCode::RequiredCountUnavailable,
                "filtered work requires an accepted count",
            ));
        }
        if let Some(action) = self.state.current_action.as_mut() {
            action.counts = current;
        }
        Ok(())
    }
}
