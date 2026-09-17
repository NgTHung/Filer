//! # Event conversion
//!
//! This private module parses event projections, preserving null-versus-missing
//! information until each event field can be checked against its phase.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

use super::validation::{PROTOCOL_VERSION, schema_error, validate_digest, validate_identifier};
use super::{
    Continuation, Counts, Event, Kind, MetricValue, Output, OutputScope, Phase, Row, Status,
    StatusKind, UnavailableReason,
};
use crate::{ErrorCode, ErrorContext, ProtocolError};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawEvent {
    protocol_version: u64,
    #[serde(rename = "type")]
    message_type: String,
    run_id: String,
    sample_id: String,
    process_id: String,
    order_id: String,
    sequence: u64,
    timestamp_ns: u64,
    phase: String,
    action_id: Presence<String>,
    counts: RawCounts,
    rows: Vec<serde_json::Value>,
    output: Presence<RawOutput>,
    metrics: StrictMap<RawMetricValue>,
    status: Presence<RawStatus>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCounts {
    examined: RawMetricValue,
    accepted: RawMetricValue,
    emitted: RawMetricValue,
    visible: RawMetricValue,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum RawMetricValue {
    Observed(u64),
    Unavailable(RawUnavailable),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnavailable {
    unavailable: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    #[serde(default)]
    identity: Presence<String>,
    #[serde(default)]
    kind: Presence<String>,
    #[serde(default)]
    size_bytes: Presence<Option<u64>>,
    #[serde(default)]
    modified_unix_ns: Presence<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutput {
    scope: String,
    digest: String,
    row_count: u64,
    continuation: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatus {
    kind: String,
    code: Presence<String>,
    message: Presence<String>,
}

#[derive(Clone, Debug, Default)]
enum Presence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> Deserialize<'de> for Presence<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PresenceVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for PresenceVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Presence<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a nullable protocol value")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Presence::Null)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Presence::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                T::deserialize(deserializer).map(Presence::Value)
            }
        }

        deserializer.deserialize_option(PresenceVisitor(std::marker::PhantomData))
    }
}

#[derive(Clone, Debug)]
struct StrictMap<V>(BTreeMap<String, V>);

impl<'de, V> Deserialize<'de> for StrictMap<V>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrictMapVisitor<V>(std::marker::PhantomData<V>);

        impl<'de, V> Visitor<'de> for StrictMapVisitor<V>
        where
            V: Deserialize<'de>,
        {
            type Value = StrictMap<V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object with unique keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some(key) = access.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate object key {key}"
                        )));
                    }
                    let value = access.next_value()?;
                    values.insert(key, value);
                }
                Ok(StrictMap(values))
            }
        }

        deserializer.deserialize_map(StrictMapVisitor(std::marker::PhantomData))
    }
}

pub(super) fn convert(raw: RawEvent) -> Result<Event, ProtocolError> {
    if raw.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::new(
            ErrorCode::UnsupportedProtocolVersion,
            "event protocol_version must be 1",
        ));
    }
    if raw.message_type != "run_event" {
        return Err(ProtocolError::new(
            ErrorCode::UnexpectedStdout,
            "standard output JSON object is not a run_event",
        ));
    }
    for (name, value) in [
        ("run_id", &raw.run_id),
        ("sample_id", &raw.sample_id),
        ("process_id", &raw.process_id),
        ("order_id", &raw.order_id),
    ] {
        validate_identifier(value).map_err(|error| error.with_field(name))?;
    }
    let phase = parse_phase(&raw.phase)?;
    let action_id = required_nullable_string(raw.action_id, "action_id")?;
    let output = required_nullable(raw.output, "output")?
        .map(convert_output)
        .transpose()?;
    let status = required_nullable(raw.status, "status")?
        .map(convert_status)
        .transpose()?;
    let rows = raw
        .rows
        .into_iter()
        .map(convert_row_value)
        .collect::<Result<Vec<_>, _>>()?;
    let metrics = raw
        .metrics
        .0
        .into_iter()
        .map(|(name, value)| {
            validate_identifier(&name)
                .map_err(|error| error.with_field(format!("metrics.{name}")))?;
            Ok((name, convert_metric(value)?))
        })
        .collect::<Result<BTreeMap<_, _>, ProtocolError>>()?;
    Ok(Event {
        protocol_version: raw.protocol_version,
        message_type: raw.message_type,
        run_id: raw.run_id,
        sample_id: raw.sample_id,
        process_id: raw.process_id,
        order_id: raw.order_id,
        sequence: raw.sequence,
        timestamp_ns: raw.timestamp_ns,
        phase,
        action_id,
        counts: convert_counts(raw.counts)?,
        rows,
        output,
        metrics,
        status,
    })
}

fn convert_counts(raw: RawCounts) -> Result<Counts, ProtocolError> {
    Ok(Counts {
        examined: convert_metric(raw.examined)?,
        accepted: convert_metric(raw.accepted)?,
        emitted: convert_metric(raw.emitted)?,
        visible: convert_metric(raw.visible)?,
    })
}

fn convert_metric(raw: RawMetricValue) -> Result<MetricValue, ProtocolError> {
    match raw {
        RawMetricValue::Observed(value) => Ok(MetricValue::Observed(value)),
        RawMetricValue::Unavailable(value) => Ok(MetricValue::Unavailable(parse_unavailable(
            &value.unavailable,
        )?)),
    }
}

fn convert_row_value(value: serde_json::Value) -> Result<Row, ProtocolError> {
    let raw: RawRow = serde_json::from_value(value).map_err(|error| {
        ProtocolError::new(
            ErrorCode::InvalidRow,
            "row does not match the canonical shape",
        )
        .with_context(ErrorContext {
            field: Some(error.to_string()),
            ..ErrorContext::default()
        })
    })?;
    let identity = match raw.identity {
        Presence::Missing => None,
        Presence::Null => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "row identity must be a string",
            ));
        }
        Presence::Value(value) => Some(value),
    };
    let kind = match raw.kind {
        Presence::Missing => None,
        Presence::Null => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "row kind must be a string",
            ));
        }
        Presence::Value(value) => Some(match value.as_str() {
            "file" => Kind::File,
            "directory" => Kind::Directory,
            _ => {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRow,
                    "row kind must be file or directory",
                ));
            }
        }),
    };
    let size_bytes = match raw.size_bytes {
        Presence::Missing => None,
        Presence::Null => Some(None),
        Presence::Value(value) => Some(value),
    };
    let modified_unix_ns = match raw.modified_unix_ns {
        Presence::Missing => None,
        Presence::Null => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "row modified_unix_ns must be an integer",
            ));
        }
        Presence::Value(value) => Some(value),
    };
    if let Some(identity) = &identity {
        validate_identity(identity)?;
    }
    if let Some(kind) = kind {
        match (kind, size_bytes) {
            (Kind::File, Some(None)) | (Kind::Directory, Some(Some(_))) => {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRow,
                    "row size_bytes does not match its kind",
                ));
            }
            _ => {}
        }
    }
    Ok(Row {
        identity,
        kind,
        size_bytes,
        modified_unix_ns,
    })
}

fn convert_output(raw: RawOutput) -> Result<Output, ProtocolError> {
    let scope = match raw.scope.as_str() {
        "membership" => OutputScope::Membership,
        "metadata" => OutputScope::Metadata,
        "ordered" => OutputScope::Ordered,
        "page" => OutputScope::Page,
        "viewport" => OutputScope::Viewport,
        _ => return Err(schema_error("output scope is not recognized")),
    };
    validate_digest(&raw.digest).map_err(|error| error.with_field("output.digest"))?;
    let continuation = match raw.continuation.as_str() {
        "more" => Continuation::More,
        "end" => Continuation::End,
        "not_applicable" => Continuation::NotApplicable,
        _ => return Err(schema_error("output continuation is not recognized")),
    };
    Ok(Output {
        scope,
        digest: raw.digest,
        row_count: raw.row_count,
        continuation,
    })
}

fn convert_status(raw: RawStatus) -> Result<Status, ProtocolError> {
    let kind = match raw.kind.as_str() {
        "success" => StatusKind::Success,
        "not_supported" => StatusKind::NotSupported,
        "error" => StatusKind::Error,
        "cancelled" => StatusKind::Cancelled,
        _ => return Err(schema_error("status kind is not recognized")),
    };
    let code = nullable_value(raw.code, "status.code")?;
    let message = nullable_value(raw.message, "status.message")?;
    match kind {
        StatusKind::Success if code.is_some() || message.is_some() => {
            return Err(schema_error("success status code and message must be null"));
        }
        StatusKind::Success => {}
        StatusKind::NotSupported | StatusKind::Error | StatusKind::Cancelled
            if code.as_deref().is_none_or(str::is_empty)
                || message.as_deref().is_none_or(str::is_empty) =>
        {
            return Err(schema_error("non-success status requires code and message"));
        }
        _ => {}
    }
    Ok(Status {
        kind,
        code,
        message,
    })
}

fn parse_phase(value: &str) -> Result<Phase, ProtocolError> {
    match value {
        "sample.started" => Ok(Phase::SampleStarted),
        "action.started" => Ok(Phase::ActionStarted),
        "row.first" => Ok(Phase::RowFirst),
        "viewport.committed" => Ok(Phase::ViewportCommitted),
        "page.committed" => Ok(Phase::PageCommitted),
        "listing.completed" => Ok(Phase::ListingCompleted),
        "transform.completed" => Ok(Phase::TransformCompleted),
        "view.committed" => Ok(Phase::ViewCommitted),
        "action.completed" => Ok(Phase::ActionCompleted),
        "sample.completed" => Ok(Phase::SampleCompleted),
        _ => Err(ProtocolError::new(
            ErrorCode::InvalidPhase,
            "phase is not recognized",
        )),
    }
}

fn parse_unavailable(value: &str) -> Result<UnavailableReason, ProtocolError> {
    match value {
        "unsupported" => Ok(UnavailableReason::Unsupported),
        "permission_denied" => Ok(UnavailableReason::PermissionDenied),
        "not_observable" => Ok(UnavailableReason::NotObservable),
        "platform_unavailable" => Ok(UnavailableReason::PlatformUnavailable),
        _ => Err(schema_error("unavailable reason is not recognized")),
    }
}

fn required_nullable<T>(value: Presence<T>, field: &str) -> Result<Option<T>, ProtocolError> {
    match value {
        Presence::Missing => Err(schema_error(format!("{field} is required"))),
        Presence::Null => Ok(None),
        Presence::Value(value) => Ok(Some(value)),
    }
}

fn required_nullable_string(
    value: Presence<String>,
    field: &str,
) -> Result<Option<String>, ProtocolError> {
    let value = required_nullable(value, field)?;
    if let Some(value) = &value {
        validate_identifier(value).map_err(|error| error.with_field(field))?;
    }
    Ok(value)
}

fn nullable_value<T>(value: Presence<T>, field: &str) -> Result<Option<T>, ProtocolError> {
    required_nullable(value, field)
}

fn validate_identity(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') || value.contains('\0') {
        return Err(ProtocolError::new(
            ErrorCode::InvalidRow,
            "identity is not a safe relative UTF-8 path",
        ));
    }
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "identity contains an unsafe path segment",
            ));
        }
    }
    Ok(())
}
