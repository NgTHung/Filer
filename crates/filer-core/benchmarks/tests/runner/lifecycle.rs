use std::path::Path;
use std::time::{Duration, Instant};

use super::support::*;

#[test]
fn accepts_a_valid_trace_and_stores_timing_with_its_identity() {
    let setup = RunnerSetup::new();

    let record = setup.replay("valid", &valid_trace(&setup).lines(), &[]);

    let SampleOutcome::Accepted(sample) = &record.outcome else {
        panic!("valid trace should be accepted");
    };
    assert!(record.outcome.timing_eligible());
    assert_eq!(sample.timeline.len(), 8);
    assert!(
        record.diagnostics.stdout_lines.is_empty(),
        "accepted samples do not retain their trace"
    );
    assert_eq!(record.diagnostics.exit_code, Some(0));
}

#[test]
fn rejects_wrong_counts_with_raw_diagnostics() {
    let setup = RunnerSetup::new();
    let mut trace = valid_trace(&setup);
    mutate_phase(&mut trace, "page.committed", |event| {
        event["counts"]["emitted"] = json!(10);
        event["counts"]["visible"] = json!(10);
    });

    let record = setup.replay(
        "short-page",
        &trace.lines(),
        &["--stderr", "replay saw a short page"],
    );

    assert_eq!(rejection_code(&record), "invalid_counts");
    assert!(!record.outcome.timing_eligible());
    let SampleOutcome::Rejected(failure) = &record.outcome else {
        panic!("sample should be rejected");
    };
    assert_eq!(failure.context.sequence, Some(4));
    assert_eq!(failure.context.action.as_deref(), Some("open"));
    assert!(
        record
            .diagnostics
            .stderr
            .contains("replay saw a short page")
    );
    let stdout = &record.diagnostics.stdout_lines;
    assert_eq!(stdout.len(), 5, "the runner stops at the rejected event");
    assert!(String::from_utf8_lossy(&stdout[4]).contains("page.committed"));
}

#[test]
fn rejects_wrong_digests() {
    let setup = RunnerSetup::new();
    let mut trace = valid_trace(&setup);
    mutate_phase(&mut trace, "listing.completed", |event| {
        event["output"]["digest"] = json!(format!("sha256:{}", "e".repeat(64)));
    });

    let record = setup.replay("wrong-digest", &trace.lines(), &[]);

    assert_eq!(rejection_code(&record), "output_digest_mismatch");
}

#[test]
fn rejects_duplicate_events() {
    let setup = RunnerSetup::new();
    let mut lines = valid_trace(&setup).lines();
    let page = lines[4].clone();
    lines.insert(5, page);

    let replayed = setup.replay("verbatim", &lines, &[]);
    let resequenced = setup.replay("resequenced", &resequence(lines), &[]);

    assert_eq!(rejection_code(&replayed), "invalid_sequence");
    assert_eq!(rejection_code(&resequenced), "duplicate_phase");
}

#[test]
fn kills_and_reaps_an_adapter_that_misses_its_deadline() {
    let setup = RunnerSetup::new();
    let lines = valid_trace(&setup).lines();
    let trace = setup.write_trace("partial", &lines[..3]);
    let timeout = Duration::from_millis(500);
    let mut runner = setup.runner(
        replay_adapter(&trace, &["--hang", "--stderr", "still listing"]),
        &[],
        timeout,
    );

    let started = Instant::now();
    let record = runner.run_sample(sample_spec()).expect("runner completes");

    assert!(started.elapsed() < timeout + Duration::from_secs(10));
    assert_eq!(rejection_code(&record), "adapter_timeout");
    assert_eq!(record.diagnostics.stdout_lines.len(), 3);
    assert!(record.diagnostics.stderr.contains("still listing"));
    assert!(
        record.diagnostics.exit_status.is_some(),
        "a killed adapter must be waited for"
    );
    assert_ne!(record.diagnostics.exit_code, Some(0));
}

#[test]
fn stops_a_running_adapter_as_soon_as_its_output_is_rejected() {
    let setup = RunnerSetup::new();
    let mut lines = valid_trace(&setup).lines();
    lines.insert(1, b"listing started\n".to_vec());
    let trace = setup.write_trace("diagnostic", &lines);
    let mut runner = setup.runner(replay_adapter(&trace, &["--hang"]), &[], LONG_TIMEOUT);

    let started = Instant::now();
    let record = runner.run_sample(sample_spec()).expect("runner completes");

    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(rejection_code(&record), "unexpected_stdout");
    assert!(record.diagnostics.exit_status.is_some());
}

#[test]
fn records_requested_metrics_that_the_adapter_cannot_observe() {
    let setup = RunnerSetup::new();
    let mut trace = valid_trace(&setup);
    mutate_phase(&mut trace, "sample.completed", |event| {
        event["metrics"] = json!({
            "cpu_time_ns": 1_500,
            "allocation_count": {"unavailable": "unsupported"}
        });
    });
    let path = setup.write_trace("metrics", &trace.lines());
    let metrics = ["cpu_time_ns", "allocation_count"];
    let mut runner = setup.runner(replay_adapter(&path, &[]), &metrics, LONG_TIMEOUT);

    let record = runner.run_sample(sample_spec()).expect("runner completes");

    assert!(record.outcome.timing_eligible());
    let SampleOutcome::Accepted(sample) = &record.outcome else {
        panic!("sample should be accepted");
    };
    assert_eq!(
        sample.metrics.get("cpu_time_ns"),
        Some(&MetricValue::Observed(1_500))
    );
    assert_eq!(
        sample.metrics.get("allocation_count"),
        Some(&MetricValue::Unavailable(UnavailableReason::Unsupported))
    );
}

#[test]
fn rejects_a_trace_that_omits_a_requested_metric() {
    let setup = RunnerSetup::new();
    let path = setup.write_trace("no-metrics", &valid_trace(&setup).lines());
    let mut runner = setup.runner(
        replay_adapter(&path, &[]),
        &["allocation_count"],
        LONG_TIMEOUT,
    );

    let record = runner.run_sample(sample_spec()).expect("runner completes");

    assert_eq!(rejection_code(&record), "invalid_schema");
    let SampleOutcome::Rejected(failure) = &record.outcome else {
        panic!("sample should be rejected");
    };
    assert_eq!(
        failure.context.field.as_deref(),
        Some("metrics.allocation_count")
    );
}

#[test]
fn rejects_a_valid_trace_from_an_adapter_that_exits_with_failure() {
    let setup = RunnerSetup::new();

    let record = setup.replay(
        "exit-3",
        &valid_trace(&setup).lines(),
        &["--exit-code", "3"],
    );

    assert_eq!(rejection_code(&record), "adapter_exit_status");
    assert_eq!(record.diagnostics.exit_code, Some(3));
}

#[test]
fn records_an_adapter_that_cannot_start() {
    let setup = RunnerSetup::new();
    let mut adapter = replay_adapter(Path::new("unused.ndjson"), &[]);
    adapter.program = setup.fixture.root().join("missing-adapter");
    let mut runner = setup.runner(adapter, &[], LONG_TIMEOUT);

    let record = runner.run_sample(sample_spec()).expect("runner completes");

    assert_eq!(rejection_code(&record), "adapter_spawn_failed");
    assert!(record.diagnostics.exit_status.is_none());
}

#[test]
fn keeps_not_supported_results_out_of_timing_and_refuses_reused_samples() {
    let setup = RunnerSetup::new();
    let mut trace = TraceBuilder::new(&[]);
    trace.start_sample();
    trace.finish_not_supported(SCENARIO);
    let path = setup.write_trace("not-supported", &trace.lines());
    let mut runner = setup.runner(replay_adapter(&path, &[]), &[], LONG_TIMEOUT);

    let record = runner.run_sample(sample_spec()).expect("runner completes");
    let reused = runner.run_sample(sample_spec());

    assert!(matches!(record.outcome, SampleOutcome::Accepted(_)));
    assert!(!record.outcome.timing_eligible());
    match reused {
        Err(RunnerError::Request(error)) => assert_eq!(error.code(), ErrorCode::DuplicateSample),
        other => panic!("reused sample should be refused, got {other:?}"),
    }
}
