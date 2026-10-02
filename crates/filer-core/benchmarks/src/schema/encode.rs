//! # Protocol encoding
//!
//! Runners write requests and adapters write events with these encoders, so a
//! wire spelling has one home shared with parsing. Encoding happens after an
//! event's timestamp is taken, which keeps serialization out of measured
//! milestones.

use serde_json::{Map, Value, json};

use super::{
    Counts, Event, Field, Filter, MetricValue, Output, Request, Row, Sort, Status,
    validation::PROTOCOL_VERSION,
};

/// Encode one newline-terminated request frame.
pub fn encode_request_line(request: &Request) -> Vec<u8> {
    line(&request_value(request))
}

pub(crate) fn request_value(request: &Request) -> Value {
    let (sort_field, sort_direction) = match request.sort {
        Sort::ProviderOrder => ("provider_order", "none"),
        Sort::NameAscending => ("name", "ascending"),
    };
    let filter = match &request.filter {
        Filter::None => json!({"kind": "none"}),
        Filter::NameContains {
            value,
            case_sensitive,
        } => json!({"kind": "name_contains", "value": value, "case_sensitive": case_sensitive}),
    };
    json!({
        "protocol_version": PROTOCOL_VERSION,
        "type": "run_request",
        "run_id": request.run_id,
        "sample_id": request.sample_id,
        "process_id": request.process_id,
        "order_id": request.order_id,
        "scenario_id": request.scenario_id,
        "fixture": {"id": request.fixture.id, "digest": request.fixture.digest},
        "implementation": {
            "id": request.implementation.id,
            "version": request.implementation.version,
            "source_revision": request.implementation.source_revision,
            "build_profile": request.implementation.build_profile,
            "binary_digest": request.implementation.binary_digest,
        },
        "adapter": {
            "id": request.adapter.id,
            "version": request.adapter.version,
            "binary_digest": request.adapter.binary_digest,
        },
        "environment": {
            "machine_profile_id": request.environment.machine_profile_id,
            "machine_profile_digest": request.environment.machine_profile_digest,
            "filesystem_profile_id": request.environment.filesystem_profile_id,
            "filesystem_profile_digest": request.environment.filesystem_profile_digest,
        },
        "cache": {
            "process": request.cache.process.as_str(),
            "filesystem": request.cache.filesystem.as_str(),
            "semantic": request.cache.semantic.as_str(),
        },
        "viewport_size": request.viewport_size,
        "page_size": request.page_size,
        "requested_fields": request
            .requested_fields
            .iter()
            .map(|field| field.as_str())
            .collect::<Vec<_>>(),
        "sort": {"field": sort_field, "direction": sort_direction},
        "filter": filter,
        "group": {"kind": "none"},
        "search": {"kind": "none"},
        "clock": {"kind": "process_monotonic", "unit": "nanosecond"},
    })
}

/// Encode one newline-terminated event frame.
pub fn encode_event_line(event: &Event) -> Vec<u8> {
    let value = json!({
        "protocol_version": PROTOCOL_VERSION,
        "type": "run_event",
        "run_id": event.run_id,
        "sample_id": event.sample_id,
        "process_id": event.process_id,
        "order_id": event.order_id,
        "sequence": event.sequence,
        "timestamp_ns": event.timestamp_ns,
        "phase": event.phase.as_str(),
        "action_id": event.action_id,
        "counts": counts_value(&event.counts),
        "rows": event.rows.iter().map(row).collect::<Vec<_>>(),
        "output": event.output.as_ref().map(output_value),
        "metrics": event
            .metrics
            .iter()
            .map(|(name, value)| (name.clone(), metric_value(value)))
            .collect::<Map<_, _>>(),
        "status": event.status.as_ref().map(status_value),
    });
    line(&value)
}

pub(crate) fn counts_value(counts: &Counts) -> Value {
    json!({
        "examined": metric_value(&counts.examined),
        "accepted": metric_value(&counts.accepted),
        "emitted": metric_value(&counts.emitted),
        "visible": metric_value(&counts.visible),
    })
}

pub(crate) fn metric_value(value: &MetricValue) -> Value {
    match value {
        MetricValue::Observed(value) => json!(value),
        MetricValue::Unavailable(reason) => json!({"unavailable": reason.as_str()}),
    }
}

fn row(row: &Row) -> Value {
    let mut value = Map::new();
    if let Some(identity) = &row.identity {
        value.insert(Field::Identity.as_str().to_string(), json!(identity));
    }
    if let Some(kind) = row.kind {
        value.insert(Field::Kind.as_str().to_string(), json!(kind.as_str()));
    }
    if let Some(size_bytes) = row.size_bytes {
        value.insert(Field::SizeBytes.as_str().to_string(), json!(size_bytes));
    }
    if let Some(modified_unix_ns) = row.modified_unix_ns {
        value.insert(
            Field::ModifiedUnixNs.as_str().to_string(),
            json!(modified_unix_ns),
        );
    }
    Value::Object(value)
}

pub(crate) fn output_value(output: &Output) -> Value {
    json!({
        "scope": output.scope.as_str(),
        "digest": output.digest,
        "row_count": output.row_count,
        "continuation": output.continuation.as_str(),
    })
}

pub(crate) fn status_value(status: &Status) -> Value {
    json!({"kind": status.kind.as_str(), "code": status.code, "message": status.message})
}

fn line(value: &Value) -> Vec<u8> {
    let mut line = value.to_string().into_bytes();
    line.push(b'\n');
    line
}
