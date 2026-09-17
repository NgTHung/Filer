//! # Protocol errors
//!
//! This module maps parsing and schema failures to stable result codes while
//! retaining enough location context for a caller to diagnose one message.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorCode {
    MalformedJson,
    UnsupportedProtocolVersion,
    InvalidSchema,
    InvalidScenarioConfiguration,
    FixtureReferenceMismatch,
    DuplicateSample,
    CorrelationMismatch,
    InvalidSequence,
    ClockRegression,
    InvalidPhase,
    InvalidAction,
    InvalidCounts,
    RequiredCountUnavailable,
    InvalidRow,
    DuplicateIdentity,
    OutputRowCountMismatch,
    OutputDigestMismatch,
    MembershipMismatch,
    MissingRequiredPhase,
    DuplicatePhase,
    InvalidStatus,
    UnsupportedReportedAsSuccess,
    UnexpectedStdout,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MalformedJson => "malformed_json",
            Self::UnsupportedProtocolVersion => "unsupported_protocol_version",
            Self::InvalidSchema => "invalid_schema",
            Self::InvalidScenarioConfiguration => "invalid_scenario_configuration",
            Self::FixtureReferenceMismatch => "fixture_reference_mismatch",
            Self::DuplicateSample => "duplicate_sample",
            Self::CorrelationMismatch => "correlation_mismatch",
            Self::InvalidSequence => "invalid_sequence",
            Self::ClockRegression => "clock_regression",
            Self::InvalidPhase => "invalid_phase",
            Self::InvalidAction => "invalid_action",
            Self::InvalidCounts => "invalid_counts",
            Self::RequiredCountUnavailable => "required_count_unavailable",
            Self::InvalidRow => "invalid_row",
            Self::DuplicateIdentity => "duplicate_identity",
            Self::OutputRowCountMismatch => "output_row_count_mismatch",
            Self::OutputDigestMismatch => "output_digest_mismatch",
            Self::MembershipMismatch => "membership_mismatch",
            Self::MissingRequiredPhase => "missing_required_phase",
            Self::DuplicatePhase => "duplicate_phase",
            Self::InvalidStatus => "invalid_status",
            Self::UnsupportedReportedAsSuccess => "unsupported_reported_as_success",
            Self::UnexpectedStdout => "unexpected_stdout",
        }
    }

    pub const fn code_name(code: &'static str) -> &'static str {
        code
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ErrorContext {
    pub line: Option<usize>,
    pub sequence: Option<u64>,
    pub action: Option<String>,
    pub field: Option<String>,
}

impl ErrorContext {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(line) = self.line {
            parts.push(format!("line {line}"));
        }
        if let Some(sequence) = self.sequence {
            parts.push(format!("sequence {sequence}"));
        }
        if let Some(action) = &self.action {
            parts.push(format!("action {action}"));
        }
        if let Some(field) = &self.field {
            parts.push(format!("field {field}"));
        }
        parts.join(", ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolError {
    code: ErrorCode,
    message: String,
    context: ErrorContext,
}

impl ProtocolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: ErrorContext::default(),
        }
    }

    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.context.line = Some(line);
        self
    }

    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.context.sequence = Some(sequence);
        self
    }

    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.context.action = Some(action.into());
        self
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.context.field = Some(field.into());
        self
    }

    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    pub const fn code_str(&self) -> &'static str {
        self.code.as_str()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn context(&self) -> &ErrorContext {
        &self.context
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code_str())?;
        let location = self.context.describe();
        if !location.is_empty() {
            write!(formatter, " ({location})")?;
        }
        write!(formatter, ": {}", self.message)
    }
}

impl std::error::Error for ProtocolError {}
