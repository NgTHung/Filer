use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use filer_core_benchmarks::{
    AdapterArgs, AdapterTrace, CanonicalRow, Continuation, Counts, DeclaredCapabilities,
    MetricValue, Milestone, OutputScope, Phase, ResourceMeter, RunValidator, Status, StatusKind,
    UnavailableReason, ValidatedManifest, encode_event_line, parse_request_bytes,
    requested_metric_values,
};

fn manifest() -> ValidatedManifest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join("flat-10k-v1.json");
    ValidatedManifest::load(path).expect("manifest should validate")
}

fn request_bytes(manifest: &ValidatedManifest) -> Vec<u8> {
    let golden = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("valid_request.json"),
    )
    .expect("golden request is readable");
    let text = String::from_utf8(golden).expect("golden request is UTF-8");
    assert!(text.contains(manifest.manifest_digest()));
    text.into_bytes()
}

fn counts(emitted: u64, visible: u64) -> Counts {
    Counts {
        examined: MetricValue::Unavailable(UnavailableReason::NotObservable),
        accepted: MetricValue::Unavailable(UnavailableReason::NotObservable),
        emitted: MetricValue::Observed(emitted),
        visible: MetricValue::Observed(visible),
    }
}

fn validate(
    manifest: ValidatedManifest,
    request: &[u8],
    events: &[filer_core_benchmarks::Event],
    metrics: &[&str],
) -> filer_core_benchmarks::ValidatedSample {
    let mut validator = RunValidator::new(
        manifest,
        DeclaredCapabilities::new(["browse.fast.first"], true, false),
        metrics.iter().map(|name| name.to_string()),
    )
    .expect("validator context should be valid");
    let mut sample = validator
        .start_sample(request)
        .expect("request should be accepted");
    for event in events {
        sample
            .ingest_line(&encode_event_line(event))
            .expect("adapter event should be accepted");
    }
    sample.finish().expect("trace should finalize")
}

fn fast_first_trace(trace: &mut AdapterTrace, rows: &[CanonicalRow]) {
    trace.sample_started();
    trace.action_started("open");
    let page_arrived = trace.now_ns();
    for (phase, visible, scope, continuation, committed) in [
        (
            Phase::RowFirst,
            1,
            OutputScope::Viewport,
            Continuation::NotApplicable,
            &rows[..1],
        ),
        (
            Phase::ViewportCommitted,
            40,
            OutputScope::Viewport,
            Continuation::NotApplicable,
            &rows[..40],
        ),
        (
            Phase::PageCommitted,
            256,
            OutputScope::Page,
            Continuation::More,
            &rows[..256],
        ),
    ] {
        trace.milestone(
            page_arrived,
            "open",
            Milestone {
                phase,
                counts: counts(256, visible),
                rows: committed.to_vec(),
                scope,
                continuation,
            },
        );
    }
    let completed = trace.now_ns();
    trace.milestone(
        completed,
        "open",
        Milestone {
            phase: Phase::ListingCompleted,
            counts: counts(rows.len() as u64, 256),
            rows: rows.to_vec(),
            scope: OutputScope::Membership,
            continuation: Continuation::NotApplicable,
        },
    );
    trace.action_completed("open", counts(rows.len() as u64, 256));
}

#[test]
fn builds_a_trace_the_validator_accepts() {
    let manifest = manifest();
    let request_bytes = request_bytes(&manifest);
    let request = parse_request_bytes(&request_bytes).expect("request is valid");
    let rows = manifest.expected_rows();
    let mut trace = AdapterTrace::new(request);

    fast_first_trace(&mut trace, &rows);
    let events = trace.finish(
        Status {
            kind: StatusKind::Success,
            code: None,
            message: None,
        },
        BTreeMap::from([("cpu_time_ns".to_string(), MetricValue::Observed(10))]),
    );

    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].timestamp_ns <= pair[1].timestamp_ns)
    );
    let terminal = events.last().expect("terminal event");
    assert_eq!(terminal.counts.emitted, MetricValue::Observed(10_000));
    assert_eq!(
        terminal.counts.examined,
        MetricValue::Unavailable(UnavailableReason::NotObservable)
    );
    let sample = validate(manifest, &request_bytes, &events, &["cpu_time_ns"]);
    assert_eq!(sample.status.kind, StatusKind::Success);
    assert_eq!(sample.timeline.len(), 8);
}

#[test]
fn ends_an_interrupted_action_with_an_error_trace() {
    let manifest = manifest();
    let request_bytes = request_bytes(&manifest);
    let request = parse_request_bytes(&request_bytes).expect("request is valid");
    let mut trace = AdapterTrace::new(request);
    trace.sample_started();
    trace.action_started("open");

    let events = trace.finish(
        Status {
            kind: StatusKind::Error,
            code: Some("core_error".to_string()),
            message: Some("scan failed".to_string()),
        },
        BTreeMap::new(),
    );

    let sample = validate(manifest, &request_bytes, &events, &[]);
    assert_eq!(sample.status.kind, StatusKind::Error);
    assert_eq!(
        events.last().map(|event| event.counts.emitted.clone()),
        Some(MetricValue::Observed(0))
    );
}

#[test]
fn parses_the_runner_arguments() {
    let args = AdapterArgs::parse(
        [
            "--fixture-root",
            "/tmp/flat-10k",
            "--metric",
            "cpu_time_ns",
            "--metric",
            "peak_rss_bytes",
        ]
        .map(OsString::from),
    )
    .expect("arguments are valid");

    assert_eq!(args.fixture_root, PathBuf::from("/tmp/flat-10k"));
    assert_eq!(args.metrics, vec!["cpu_time_ns", "peak_rss_bytes"]);
    for invalid in [
        vec!["--metric", "cpu_time_ns"],
        vec!["--fixture-root"],
        vec!["--fixture-root", "/tmp/a", "--verbose"],
    ] {
        assert!(AdapterArgs::parse(invalid.into_iter().map(OsString::from)).is_err());
    }
}

#[test]
fn reports_every_requested_metric_and_marks_unknown_names_unsupported() {
    let meter = ResourceMeter::start();
    let report = meter.finish();
    let requested = [
        "cpu_time_ns".to_string(),
        "peak_rss_bytes".to_string(),
        "core_event_count".to_string(),
        "allocation_count".to_string(),
    ];

    let values = requested_metric_values(&requested, |name| match name {
        "core_event_count" => Some(MetricValue::Observed(42)),
        _ => report.get(name),
    });

    assert_eq!(values.len(), 4);
    assert_eq!(values["core_event_count"], MetricValue::Observed(42));
    assert_eq!(
        values["allocation_count"],
        MetricValue::Unavailable(UnavailableReason::Unsupported)
    );
    if cfg!(unix) {
        assert!(matches!(values["cpu_time_ns"], MetricValue::Observed(_)));
        assert!(matches!(values["peak_rss_bytes"], MetricValue::Observed(bytes) if bytes > 0));
    } else {
        assert_eq!(
            values["cpu_time_ns"],
            MetricValue::Unavailable(UnavailableReason::PlatformUnavailable)
        );
    }
}
