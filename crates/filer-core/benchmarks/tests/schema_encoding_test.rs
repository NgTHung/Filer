use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use filer_core_benchmarks::{
    Continuation, Counts, Event, Kind, MetricValue, Output, OutputScope, Phase, Row,
    UnavailableReason, encode_event_line, encode_request_line, parse_event_line, parse_event_lines,
    parse_request_bytes,
};

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    fs::read(path).unwrap_or_else(|error| panic!("golden input should be readable: {error}"))
}

fn assert_single_line(bytes: &[u8]) {
    assert!(
        bytes.ends_with(b"\n"),
        "encoded frame must end with newline"
    );
    assert_eq!(
        bytes.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "encoded frame must be exactly one line"
    );
}

#[test]
fn encodes_the_valid_request_golden_as_an_equivalent_request() {
    let request = parse_request_bytes(&golden("valid_request.json")).expect("request is valid");

    let encoded = encode_request_line(&request);

    assert_single_line(&encoded);
    assert_eq!(
        parse_request_bytes(&encoded).expect("encoded request parses"),
        request
    );
}

#[test]
fn encodes_name_sort_and_filter_requests_as_equivalent_requests() {
    let source = String::from_utf8(golden("valid_request.json")).expect("golden is UTF-8");
    let source = source
        .replacen(
            r#""sort":{"field":"provider_order","direction":"none"}"#,
            r#""sort":{"field":"name","direction":"ascending"}"#,
            1,
        )
        .replacen(
            r#""filter":{"kind":"none"}"#,
            r#""filter":{"kind":"name_contains","value":"file-0001","case_sensitive":true}"#,
            1,
        )
        .replacen(
            r#""cache":{"process":"cold","filesystem":"warm","semantic":"empty"}"#,
            r#""cache":{"process":"warm","filesystem":"fresh_copy","semantic":"reused"}"#,
            1,
        );
    let request = parse_request_bytes(source.as_bytes()).expect("mutated request is valid");

    let encoded = encode_request_line(&request);

    assert_eq!(
        parse_request_bytes(&encoded).expect("encoded request parses"),
        request
    );
}

#[test]
fn encodes_trace_goldens_as_equivalent_events() {
    for name in ["not_supported_trace.ndjson", "invalid_page_event.ndjson"] {
        let events = parse_event_lines(&golden(name)).expect("golden trace parses");
        for event in events {
            let encoded = encode_event_line(&event);

            assert_single_line(&encoded);
            assert_eq!(
                parse_event_line(&encoded).expect("encoded event parses"),
                event,
                "{name} event should round-trip"
            );
        }
    }
}

#[test]
fn encodes_metadata_rows_and_unavailable_values() {
    let event = Event {
        protocol_version: 1,
        message_type: "run_event".to_string(),
        run_id: "run-local-001".to_string(),
        sample_id: "sample-0001".to_string(),
        process_id: "process-0001".to_string(),
        order_id: "round-01-position-02".to_string(),
        sequence: 2,
        timestamp_ns: 7_000,
        phase: Phase::ViewportCommitted,
        action_id: Some("open".to_string()),
        counts: Counts {
            examined: MetricValue::Unavailable(UnavailableReason::NotObservable),
            accepted: MetricValue::Unavailable(UnavailableReason::NotObservable),
            emitted: MetricValue::Observed(2),
            visible: MetricValue::Observed(2),
        },
        rows: vec![
            Row {
                identity: Some(".dir-000000".to_string()),
                kind: Some(Kind::Directory),
                size_bytes: Some(None),
                modified_unix_ns: Some(1_704_067_200_000_000_000),
            },
            Row {
                identity: Some("file-000001.txt".to_string()),
                kind: Some(Kind::File),
                size_bytes: Some(Some(7_920)),
                modified_unix_ns: Some(-1),
            },
        ],
        output: Some(Output {
            scope: OutputScope::Viewport,
            digest: format!("sha256:{}", "a".repeat(64)),
            row_count: 2,
            continuation: Continuation::NotApplicable,
        }),
        metrics: BTreeMap::from([
            ("cpu_time_ns".to_string(), MetricValue::Observed(15)),
            (
                "allocation_count".to_string(),
                MetricValue::Unavailable(UnavailableReason::Unsupported),
            ),
        ]),
        status: None,
    };

    let encoded = encode_event_line(&event);

    assert_eq!(
        parse_event_line(&encoded).expect("encoded event parses"),
        event
    );
    let text = String::from_utf8(encoded).expect("encoded event is UTF-8");
    assert!(text.contains(r#""size_bytes":null"#));
    assert!(text.contains(r#"{"unavailable":"not_observable"}"#));
}
