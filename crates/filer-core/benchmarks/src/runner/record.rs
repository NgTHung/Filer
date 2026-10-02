//! # Raw sample records
//!
//! A raw record is the reproducible source for every later report. It keeps
//! the exact request, the full profile records behind its digests, the fixture
//! identity, and either the validated row-free timeline or the rejected
//! adapter's raw output. Records are created exclusively so an earlier result
//! is never replaced by a rerun.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use super::{AdapterDiagnostics, RunPlan, RunnerError, SampleOutcome};
use crate::ErrorContext;
use crate::fixtures::PreparedFixture;
use crate::profile::ProfileRecord;
use crate::schema::{Request, encode};
use crate::validator::{TimelineEntry, ValidatedSample};

const RECORD_SCHEMA: &str = "filer-benchmark-raw-sample-v1";

pub(super) struct RecordInput<'input> {
    pub(super) plan: &'input RunPlan,
    pub(super) fixture: &'input PreparedFixture,
    pub(super) request: &'input Request,
    /// Runner wall-clock bounds; event timestamps stay in the adapter clock.
    pub(super) wall_clock: (Option<u64>, Option<u64>),
    pub(super) outcome: &'input SampleOutcome,
    pub(super) diagnostics: &'input AdapterDiagnostics,
}

pub(super) fn raw_record(input: &RecordInput<'_>) -> Value {
    let plan = input.plan;
    let adapter = &plan.adapter;
    let mut requested_metrics = plan.requested_metrics.clone();
    requested_metrics.sort();
    requested_metrics.dedup();
    json!({
        "schema": RECORD_SCHEMA,
        "runner_wall_clock": {
            "started_unix_ns": input.wall_clock.0,
            "finished_unix_ns": input.wall_clock.1,
        },
        "request": encode::request_value(input.request),
        "requested_metrics": requested_metrics,
        "fixture": {
            "id": input.fixture.manifest_id(),
            "digest": input.fixture.manifest_digest(),
            "root": input.fixture.root().display().to_string(),
        },
        "profiles": {
            "machine": profile(&plan.machine),
            "filesystem": profile(&plan.filesystem),
            "build": profile(&plan.build),
            "adapter": {
                "id": adapter.identity.id,
                "version": adapter.identity.version,
                "binary_digest": adapter.identity.binary_digest,
                "program": adapter.program.display().to_string(),
                "args": adapter.args,
                "capabilities": {
                    "scenarios": adapter.capabilities.supported_scenarios().collect::<Vec<_>>(),
                    "streaming_unfiltered_listing":
                        adapter.capabilities.streaming_unfiltered_listing,
                    "examined_count_observable": adapter.capabilities.examined_count_observable,
                },
            },
        },
        "outcome": outcome(input.outcome),
        "diagnostics": diagnostics(input.diagnostics),
    })
}

/// Writes `<results>/<run_id>/<sample_id>.json`, refusing to replace a file.
pub(super) fn write_new(
    results_dir: &Path,
    request: &Request,
    value: &Value,
) -> Result<PathBuf, RunnerError> {
    let directory = results_dir.join(&request.run_id);
    let path = directory.join(format!("{}.json", request.sample_id));
    let results_error = |source| RunnerError::Results {
        path: path.clone(),
        source,
    };
    fs::create_dir_all(&directory).map_err(results_error)?;
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| results_error(std::io::Error::other(error)))?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(results_error)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(results_error)?;
    Ok(path)
}

pub(super) fn unix_now_ns() -> Option<u64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_nanos()).ok()
}

fn profile(record: &ProfileRecord) -> Value {
    json!({
        "id": record.id(),
        "digest": record.digest(),
        "entries": record
            .entries()
            .iter()
            .map(|(name, value)| json!({"name": name, "value": value}))
            .collect::<Vec<_>>(),
    })
}

fn outcome(outcome: &SampleOutcome) -> Value {
    match outcome {
        SampleOutcome::Accepted(sample) => accepted(sample, outcome.timing_eligible()),
        SampleOutcome::Rejected(failure) => json!({
            "result": "rejected",
            "code": failure.code_str(),
            "message": failure.message,
            "context": context(&failure.context),
        }),
    }
}

fn accepted(sample: &ValidatedSample, timing_eligible: bool) -> Value {
    json!({
        "result": "accepted",
        "status": encode::status_value(&sample.status),
        "timing_eligible": timing_eligible,
        "structural_gates": {
            "first_page_examined": sample.structural_gates.first_page_examined.as_str(),
        },
        "metrics": sample
            .metrics
            .iter()
            .map(|(name, value)| (name.clone(), encode::metric_value(value)))
            .collect::<Map<_, _>>(),
        "timeline": sample.timeline.iter().map(timeline_entry).collect::<Vec<_>>(),
    })
}

fn timeline_entry(entry: &TimelineEntry) -> Value {
    json!({
        "sequence": entry.sequence,
        "timestamp_ns": entry.timestamp_ns,
        "phase": entry.phase.as_str(),
        "action_id": entry.action_id,
        "counts": encode::counts_value(&entry.counts),
        "output": entry.output.as_ref().map(encode::output_value),
    })
}

fn context(context: &ErrorContext) -> Value {
    json!({
        "line": context.line,
        "sequence": context.sequence,
        "action": context.action,
        "field": context.field,
    })
}

fn diagnostics(diagnostics: &AdapterDiagnostics) -> Value {
    let mut value = json!({
        "exit_status": diagnostics.exit_status,
        "exit_code": diagnostics.exit_code,
        "stderr": diagnostics.stderr,
        "stderr_truncated": diagnostics.stderr_truncated,
    });
    if !diagnostics.stdout_lines.is_empty()
        && let Value::Object(map) = &mut value
    {
        map.insert(
            "stdout_lines".to_string(),
            diagnostics
                .stdout_lines
                .iter()
                .map(|line| Value::String(String::from_utf8_lossy(line).into_owned()))
                .collect(),
        );
    }
    value
}
