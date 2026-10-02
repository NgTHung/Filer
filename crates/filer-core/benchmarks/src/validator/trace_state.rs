//! # Trace state
//!
//! This private module stores the mutable counters and milestone observations
//! shared by the validator's event transitions.

use std::collections::BTreeSet;

use crate::canonical::CanonicalRow;
use crate::scenarios::ActionPlan;
use crate::schema::{Event, MetricValue, Phase, Status};
use crate::{ErrorCode, ProtocolError};

#[derive(Clone, Debug)]
pub(super) struct TraceState {
    pub(super) line: usize,
    pub(super) events: usize,
    pub(super) next_sequence: u64,
    pub(super) last_timestamp: Option<u64>,
    pub(super) started: bool,
    pub(super) completed: bool,
    pub(super) next_action_index: usize,
    pub(super) current_action: Option<ActionState>,
    pub(super) terminal_status: Option<Status>,
    pub(super) totals: CountsState,
    pub(super) page_chain: BTreeSet<String>,
    pub(super) gate: super::GateResult,
}

impl TraceState {
    pub(super) fn new(gate: super::GateResult) -> Self {
        Self {
            line: 0,
            events: 0,
            next_sequence: 0,
            last_timestamp: None,
            started: false,
            completed: false,
            next_action_index: 0,
            current_action: None,
            terminal_status: None,
            totals: CountsState::zero(),
            page_chain: BTreeSet::new(),
            gate,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ActionState {
    pub(super) plan: ActionPlan,
    pub(super) seen: Vec<Phase>,
    pub(super) counts: CountsState,
    pub(super) row_first: Option<Vec<CanonicalRow>>,
    pub(super) viewport: Option<Vec<CanonicalRow>>,
    pub(super) page: Option<Vec<CanonicalRow>>,
}

impl ActionState {
    pub(super) fn new(plan: ActionPlan) -> Self {
        Self {
            plan,
            seen: Vec::new(),
            counts: CountsState::zero(),
            row_first: None,
            viewport: None,
            page: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CountsState {
    pub(super) examined: Option<u64>,
    pub(super) accepted: Option<u64>,
    pub(super) emitted: Option<u64>,
    pub(super) visible: Option<u64>,
}

impl CountsState {
    pub(super) fn zero() -> Self {
        Self {
            examined: Some(0),
            accepted: Some(0),
            emitted: Some(0),
            visible: Some(0),
        }
    }

    pub(super) fn from_event(event: &Event) -> Self {
        Self {
            examined: observed(&event.counts.examined),
            accepted: observed(&event.counts.accepted),
            emitted: observed(&event.counts.emitted),
            visible: observed(&event.counts.visible),
        }
    }

    pub(super) fn is_zero(&self) -> bool {
        self.examined == Some(0)
            && self.accepted == Some(0)
            && self.emitted == Some(0)
            && self.visible == Some(0)
    }

    pub(super) fn validate_progress(&self, previous: &Self) -> Result<(), ProtocolError> {
        for (before, after) in [
            (previous.examined, self.examined),
            (previous.accepted, self.accepted),
            (previous.emitted, self.emitted),
            (previous.visible, self.visible),
        ] {
            if let (Some(before), Some(after)) = (before, after)
                && after < before
            {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidCounts,
                    "action counts decreased",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_relations(&self) -> Result<(), ProtocolError> {
        if let (Some(accepted), Some(examined)) = (self.accepted, self.examined)
            && accepted > examined
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidCounts,
                "accepted cannot exceed examined",
            ));
        }
        if let (Some(emitted), Some(accepted)) = (self.emitted, self.accepted)
            && emitted > accepted
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidCounts,
                "emitted cannot exceed accepted",
            ));
        }
        if let (Some(visible), Some(emitted)) = (self.visible, self.emitted)
            && visible > emitted
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidCounts,
                "visible cannot exceed emitted",
            ));
        }
        Ok(())
    }

    pub(super) fn add(&mut self, action: &Self) -> Result<(), ProtocolError> {
        self.examined = add_values(self.examined, action.examined)?;
        self.accepted = add_values(self.accepted, action.accepted)?;
        self.emitted = add_values(self.emitted, action.emitted)?;
        self.visible = add_values(self.visible, action.visible)?;
        Ok(())
    }

    pub(super) fn matches(&self, actual: &Self) -> bool {
        [
            (self.examined, actual.examined),
            (self.accepted, actual.accepted),
            (self.emitted, actual.emitted),
            (self.visible, actual.visible),
        ]
        .into_iter()
        .all(|(expected, actual)| expected == actual)
    }
}

fn observed(value: &MetricValue) -> Option<u64> {
    match value {
        MetricValue::Observed(value) => Some(*value),
        MetricValue::Unavailable(_) => None,
    }
}

fn add_values(left: Option<u64>, right: Option<u64>) -> Result<Option<u64>, ProtocolError> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.checked_add(right)).ok_or_else(|| {
            ProtocolError::new(ErrorCode::InvalidCounts, "sample count total overflowed")
        }),
        _ => Ok(None),
    }
}

pub(super) fn check_visible_prefix(action: &ActionState) -> Result<(), ProtocolError> {
    if let (Some(viewport), Some(page)) = (&action.viewport, &action.page)
        && (viewport.len() > page.len() || viewport != &page[..viewport.len()])
    {
        return Err(ProtocolError::new(
            ErrorCode::MembershipMismatch,
            "viewport is not the visible prefix of the observed page",
        ));
    }
    if let (Some(first), Some(viewport)) = (&action.row_first, &action.viewport)
        && first.first().map(|row| &row.identity) != viewport.first().map(|row| &row.identity)
    {
        return Err(ProtocolError::new(
            ErrorCode::MembershipMismatch,
            "row.first is not the first visible row",
        ));
    }
    Ok(())
}
