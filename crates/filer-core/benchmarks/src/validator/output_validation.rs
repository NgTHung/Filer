//! # Benchmark output validation
//!
//! This module validates canonical rows, digests, continuations, and visible
//! prefixes after the trace lifecycle has admitted a milestone.

use std::collections::BTreeSet;

use super::trace_state::{ActionState, check_visible_prefix, scope_name};
use super::{GateResult, SampleValidator};
use crate::canonical::{CanonicalRow, canonical_digest};
use crate::scenarios::{ActionKind, ListingKind};
use crate::schema::{Continuation, Event, Field, OutputScope, Phase, Row};
use crate::{ErrorCode, ProtocolError};

impl SampleValidator<'_> {
    pub(super) fn validate_rows(&self, rows: &[Row]) -> Result<Vec<CanonicalRow>, ProtocolError> {
        let mut identities = BTreeSet::new();
        let mut canonical = Vec::with_capacity(rows.len());
        for row in rows {
            let identity = row.identity.as_deref().ok_or_else(|| {
                ProtocolError::new(ErrorCode::InvalidRow, "row is missing identity")
            })?;
            let kind = row
                .kind
                .ok_or_else(|| ProtocolError::new(ErrorCode::InvalidRow, "row is missing kind"))?;
            if !identities.insert(identity.to_string()) {
                return Err(ProtocolError::new(
                    ErrorCode::DuplicateIdentity,
                    "one event contains a duplicate row identity",
                ));
            }
            let expected = self.manifest.expected_row(identity).ok_or_else(|| {
                ProtocolError::new(
                    ErrorCode::MembershipMismatch,
                    "row identity is not in the selected fixture manifest",
                )
            })?;
            if kind != expected.kind {
                return Err(ProtocolError::new(
                    ErrorCode::MembershipMismatch,
                    "row kind does not match the fixture manifest",
                ));
            }
            let has_size = self.request.requested_fields.contains(&Field::SizeBytes);
            let has_modified = self
                .request
                .requested_fields
                .contains(&Field::ModifiedUnixNs);
            if has_size != row.size_bytes.is_some()
                || has_modified != row.modified_unix_ns.is_some()
            {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRow,
                    "row fields do not match requested_fields",
                ));
            }
            let size_bytes = match row.size_bytes {
                Some(value) => value,
                None => expected.size_bytes,
            };
            let modified_unix_ns = row.modified_unix_ns.unwrap_or(expected.modified_unix_ns);
            if has_size && size_bytes != expected.size_bytes {
                return Err(ProtocolError::new(
                    ErrorCode::MembershipMismatch,
                    "row size_bytes does not match the fixture manifest",
                ));
            }
            if has_modified && modified_unix_ns != expected.modified_unix_ns {
                return Err(ProtocolError::new(
                    ErrorCode::MembershipMismatch,
                    "row modified_unix_ns does not match the fixture manifest",
                ));
            }
            canonical.push(CanonicalRow::new(
                identity,
                kind,
                size_bytes,
                modified_unix_ns,
            ));
        }
        Ok(canonical)
    }

    pub(super) fn validate_milestone_output(
        &self,
        event: &Event,
        rows: &[CanonicalRow],
    ) -> Result<(), ProtocolError> {
        let action = self.state.current_action.as_ref().ok_or_else(|| {
            ProtocolError::new(ErrorCode::InvalidAction, "milestone has no action")
        })?;
        let (scope, continuation, expected_count) =
            self.output_expectation(&action.plan.kind, event.phase)?;
        let output = event.output.as_ref().ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::OutputRowCountMismatch,
                "row-carrying phase requires output",
            )
        })?;
        if output.row_count != rows.len() as u64 || output.row_count != expected_count {
            return Err(ProtocolError::new(
                ErrorCode::OutputRowCountMismatch,
                "output row_count does not match complete event rows",
            ));
        }
        if output.scope != scope || output.continuation != continuation {
            return Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "output scope or continuation does not match the milestone",
            ));
        }
        let fields = match scope {
            OutputScope::Membership => vec![Field::Identity],
            OutputScope::Metadata => vec![
                Field::Identity,
                Field::Kind,
                Field::SizeBytes,
                Field::ModifiedUnixNs,
            ],
            _ => self.request.requested_fields.clone(),
        };
        let calculated = canonical_digest(scope_name(scope), &fields, rows);
        if calculated != output.digest {
            return Err(ProtocolError::new(
                ErrorCode::OutputDigestMismatch,
                "output digest does not match its complete rows",
            ));
        }
        if let Some(expected_digest) = self.expected_digest(&action.plan.kind, event.phase, scope)
            && expected_digest != output.digest
        {
            return Err(ProtocolError::new(
                ErrorCode::OutputDigestMismatch,
                "output digest does not match the normative scenario digest",
            ));
        }
        match event.phase {
            Phase::ListingCompleted => self.validate_listing(action, rows),
            Phase::TransformCompleted | Phase::ViewCommitted => {
                self.validate_ordered_view(&action.plan.kind, event.phase, rows)
            }
            _ => Ok(()),
        }
    }

    fn output_expectation(
        &self,
        kind: &ActionKind,
        phase: Phase,
    ) -> Result<(OutputScope, Continuation, u64), ProtocolError> {
        match phase {
            Phase::RowFirst => Ok((OutputScope::Viewport, Continuation::NotApplicable, 1)),
            Phase::ViewportCommitted | Phase::ViewCommitted => Ok((
                OutputScope::Viewport,
                Continuation::NotApplicable,
                self.viewport_count(kind),
            )),
            Phase::PageCommitted => {
                let page_number = match kind {
                    ActionKind::Open { page_number, .. } | ActionKind::Page { page_number } => {
                        *page_number
                    }
                    _ => {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidPhase,
                            "page commit is not a page action",
                        ));
                    }
                };
                let count = self.page_count(page_number)?;
                let continuation = if page_number == 40 || count < self.request.page_size {
                    Continuation::End
                } else {
                    Continuation::More
                };
                Ok((OutputScope::Page, continuation, count))
            }
            Phase::ListingCompleted => {
                let scope = match kind {
                    ActionKind::Open { listing, .. } => match listing {
                        ListingKind::Metadata => OutputScope::Metadata,
                        ListingKind::Membership => OutputScope::Membership,
                        ListingKind::None => OutputScope::Membership,
                    },
                    ActionKind::Page { .. } | ActionKind::Refresh => OutputScope::Membership,
                    _ => {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidPhase,
                            "listing is not allowed here",
                        ));
                    }
                };
                Ok((
                    scope,
                    Continuation::NotApplicable,
                    self.manifest.entry_count() as u64,
                ))
            }
            Phase::TransformCompleted => {
                let count = self.expected_ordered_count(kind);
                Ok((OutputScope::Ordered, Continuation::NotApplicable, count))
            }
            _ => Err(ProtocolError::new(
                ErrorCode::InvalidPhase,
                "phase does not carry an output",
            )),
        }
    }

    fn viewport_count(&self, kind: &ActionKind) -> u64 {
        match kind {
            ActionKind::FilterName => self
                .expected_ordered_count(kind)
                .min(self.request.viewport_size),
            _ => self.request.viewport_size,
        }
    }

    fn expected_ordered_count(&self, kind: &ActionKind) -> u64 {
        match kind {
            ActionKind::FilterName => self.manifest.expected().filter_count.unwrap_or(0),
            _ => self.manifest.entry_count() as u64,
        }
    }

    fn page_count(&self, page_number: u64) -> Result<u64, ProtocolError> {
        let page_size = self.request.page_size;
        let start = page_number
            .checked_sub(1)
            .and_then(|value| value.checked_mul(page_size))
            .ok_or_else(|| {
                ProtocolError::new(
                    ErrorCode::InvalidScenarioConfiguration,
                    "page number overflowed",
                )
            })?;
        let count = (self.manifest.entry_count() as u64)
            .saturating_sub(start)
            .min(page_size);
        if count == 0 {
            return Err(ProtocolError::new(
                ErrorCode::InvalidScenarioConfiguration,
                "page is outside the manifest",
            ));
        }
        Ok(count)
    }

    fn validate_listing(
        &self,
        action: &ActionState,
        rows: &[CanonicalRow],
    ) -> Result<(), ProtocolError> {
        if rows.len() != self.manifest.entry_count() {
            return Err(ProtocolError::new(
                ErrorCode::OutputRowCountMismatch,
                "listing proof does not contain the manifest row count",
            ));
        }
        let expected = self.manifest.expected_rows();
        let actual = rows
            .iter()
            .map(|row| row.identity.clone())
            .collect::<BTreeSet<_>>();
        let expected_ids = expected
            .iter()
            .map(|row| row.identity.clone())
            .collect::<BTreeSet<_>>();
        if actual != expected_ids {
            return Err(ProtocolError::new(
                ErrorCode::MembershipMismatch,
                "listing membership does not match the manifest",
            ));
        }
        if matches!(action.plan.kind, ActionKind::Page { page_number: 40 })
            && (self.state.page_chain.len() != self.manifest.entry_count()
                || self.state.page_chain != expected_ids)
        {
            return Err(ProtocolError::new(
                ErrorCode::MembershipMismatch,
                "continuation pages do not form complete manifest membership",
            ));
        }
        Ok(())
    }

    fn expected_digest(&self, kind: &ActionKind, phase: Phase, scope: OutputScope) -> Option<&str> {
        match scope {
            OutputScope::Membership => Some(self.manifest.membership_digest()),
            OutputScope::Metadata => Some(self.manifest.metadata_digest()),
            OutputScope::Ordered => match kind {
                ActionKind::SortName | ActionKind::ClearFilter | ActionKind::Refresh => {
                    Some(self.manifest.name_order_digest())
                }
                ActionKind::FilterName => self.manifest.filter_order_digest(),
                _ => None,
            },
            OutputScope::Viewport if phase == Phase::ViewCommitted => match kind {
                ActionKind::SortName | ActionKind::ClearFilter | ActionKind::Refresh => {
                    Some(self.manifest.name_viewport_digest())
                }
                ActionKind::FilterName => self.manifest.filter_viewport_digest(),
                _ => None,
            },
            _ => None,
        }
    }

    fn validate_ordered_view(
        &self,
        kind: &ActionKind,
        phase: Phase,
        rows: &[CanonicalRow],
    ) -> Result<(), ProtocolError> {
        let expected = self.ordered_rows(kind);
        let expected = if phase == Phase::ViewCommitted {
            &expected[..self.viewport_count(kind) as usize]
        } else {
            &expected[..]
        };
        if rows != expected {
            return Err(ProtocolError::new(
                ErrorCode::MembershipMismatch,
                "ordered output or visible prefix does not match the scenario",
            ));
        }
        Ok(())
    }

    fn ordered_rows(&self, kind: &ActionKind) -> Vec<CanonicalRow> {
        match kind {
            ActionKind::FilterName => self.manifest.expected_filter_rows().unwrap_or_default(),
            _ => self.manifest.expected_name_rows(),
        }
    }

    pub(super) fn record_milestone(
        &mut self,
        phase: Phase,
        rows: Vec<CanonicalRow>,
    ) -> Result<(), ProtocolError> {
        if self.state.current_action.is_none() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidAction,
                "milestone has no action",
            ));
        }
        match phase {
            Phase::RowFirst => {
                if let Some(action) = self.state.current_action.as_mut() {
                    action.row_first = Some(rows);
                }
            }
            Phase::ViewportCommitted => {
                if let Some(action) = self.state.current_action.as_mut() {
                    action.viewport = Some(rows);
                }
            }
            Phase::PageCommitted => {
                for row in &rows {
                    if !self.state.page_chain.insert(row.identity.clone()) {
                        return Err(ProtocolError::new(
                            ErrorCode::DuplicateIdentity,
                            "continuation pages repeat an identity",
                        ));
                    }
                }
                if let Some(action) = self.state.current_action.as_mut() {
                    action.page = Some(rows);
                }
                self.observe_first_page_gate()?;
            }
            Phase::ListingCompleted | Phase::TransformCompleted | Phase::ViewCommitted => {}
            _ => {}
        }
        let action = self.state.current_action.as_ref().ok_or_else(|| {
            ProtocolError::new(ErrorCode::InvalidAction, "milestone has no action")
        })?;
        check_visible_prefix(action)
    }

    fn observe_first_page_gate(&mut self) -> Result<(), ProtocolError> {
        let is_first_page = self.state.current_action.as_ref().is_some_and(|action| {
            matches!(action.plan.kind, ActionKind::Open { page_number: 1, .. })
        });
        if self.state.gate != GateResult::NotEvaluable
            || !self.plan.first_page_gate
            || !is_first_page
        {
            return Ok(());
        }
        if !self.capabilities.streaming_unfiltered_listing {
            self.state.gate = GateResult::NotApplicable;
            return Ok(());
        }
        if !self.capabilities.examined_count_observable {
            return Ok(());
        }
        let examined = self
            .state
            .current_action
            .as_ref()
            .and_then(|action| action.counts.examined);
        self.state.gate = match examined {
            Some(value) if value <= 512 => GateResult::Passed,
            Some(_) => GateResult::Failed,
            None => GateResult::NotEvaluable,
        };
        Ok(())
    }
}
