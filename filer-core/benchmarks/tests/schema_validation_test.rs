use std::fs;
use std::path::PathBuf;

use filer_core_benchmarks::{
    ErrorCode, Field, MetricValue, Phase, ProtocolError, parse_event_lines, parse_request_bytes,
};

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    fs::read(path).unwrap_or_else(|error| panic!("golden input should be readable: {error}"))
}

fn request_text() -> String {
    String::from_utf8(golden("valid_request.json")).expect("golden request is UTF-8")
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "mutation target must be unique"
    );
    source.replacen(from, to, 1)
}

fn schema_error(source: String) -> ProtocolError {
    parse_request_bytes(source.as_bytes()).expect_err("mutation must be rejected")
}

#[test]
fn parses_the_valid_request_golden() {
    let request = parse_request_bytes(&golden("valid_request.json")).expect("request is valid");

    assert_eq!(request.protocol_version, 1);
    assert_eq!(request.scenario_id, "browse.fast.first");
    assert_eq!(request.requested_fields, vec![Field::Identity, Field::Kind]);
}

#[test]
fn parses_the_complete_not_supported_trace_golden() {
    let events = parse_event_lines(&golden("not_supported_trace.ndjson"))
        .expect("not-supported trace is valid");

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].phase, Phase::SampleStarted);
    assert_eq!(events[1].phase, Phase::SampleCompleted);
    assert!(matches!(
        events[1].metrics.get("cpu_time_ns"),
        Some(MetricValue::Unavailable(_))
    ));
}

#[test]
fn invalid_request_golden_uses_the_stable_schema_code() {
    let error =
        parse_request_bytes(&golden("invalid_request.json")).expect_err("request is invalid");

    assert_eq!(error.code(), ErrorCode::InvalidSchema);
    assert!(error.message().contains("JSON"));
}

#[test]
fn invalid_page_golden_keeps_the_count_error_for_later_trace_validation() {
    let events = parse_event_lines(&golden("invalid_page_event.ndjson"))
        .expect("schema is valid before row-count validation");

    assert_eq!(events[0].phase, Phase::PageCommitted);
    assert_eq!(events[0].rows.len(), 1);
    assert_eq!(
        events[0].output.as_ref().map(|output| output.row_count),
        Some(256)
    );
}

#[test]
fn request_framing_requires_one_terminated_object() {
    let valid = golden("valid_request.json");
    let mut truncated = valid.clone();
    truncated.pop();
    assert_eq!(
        parse_request_bytes(&truncated).unwrap_err().code(),
        ErrorCode::MalformedJson
    );

    let mut two_objects = valid;
    two_objects.extend_from_slice(
        br#"{}
"#,
    );
    assert_eq!(
        parse_request_bytes(&two_objects).unwrap_err().code(),
        ErrorCode::MalformedJson
    );
}

#[test]
fn duplicate_metric_names_are_rejected_without_map_loss() {
    let source = br#"{"protocol_version":1,"type":"run_event","run_id":"run-1","sample_id":"sample-1","process_id":"process-1","order_id":"order-1","sequence":0,"timestamp_ns":1,"phase":"sample.started","action_id":null,"counts":{"examined":0,"accepted":0,"emitted":0,"visible":0},"rows":[],"output":null,"metrics":{"cpu_time_ns":1,"cpu_time_ns":2},"status":null}
"#;

    let error =
        filer_core_benchmarks::parse_event_line(source).expect_err("duplicate metric must fail");
    assert_eq!(error.code(), ErrorCode::InvalidSchema);
}

#[test]
fn duplicate_row_fields_are_rejected_without_map_loss() {
    let source = br#"{"protocol_version":1,"type":"run_event","run_id":"run-1","sample_id":"sample-1","process_id":"process-1","order_id":"order-1","sequence":0,"timestamp_ns":1,"phase":"page.committed","action_id":"open","counts":{"examined":1,"accepted":1,"emitted":1,"visible":1},"rows":[{"identity":"file-000000.dat","identity":"file-000001.dat","kind":"file"}],"output":{"scope":"page","digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000","row_count":1,"continuation":"more"},"metrics":{},"status":null}
"#;

    let error =
        filer_core_benchmarks::parse_event_line(source).expect_err("duplicate row field must fail");
    assert_eq!(error.code(), ErrorCode::InvalidSchema);
}

#[test]
fn plain_stdout_is_not_treated_as_a_protocol_event() {
    let error = filer_core_benchmarks::parse_event_line(b"diagnostic on stdout\n")
        .expect_err("diagnostic must fail");
    assert_eq!(error.code(), ErrorCode::UnexpectedStdout);
}

#[test]
fn non_event_json_is_not_treated_as_a_protocol_event() {
    let source = br#"{"type":"diagnostic","message":"adapter is warming up"}
"#;

    let error = filer_core_benchmarks::parse_event_line(source)
        .expect_err("non-event JSON on stdout must fail");
    assert_eq!(error.code(), ErrorCode::UnexpectedStdout);
}

#[test]
fn protocol_error_keeps_location_context() {
    let error = ProtocolError::new(ErrorCode::InvalidSchema, "bad field")
        .with_line(3)
        .with_sequence(4)
        .with_action("open")
        .with_field("rows");

    assert_eq!(error.context().line, Some(3));
    assert!(error.to_string().contains("line 3"));
    assert!(error.to_string().contains("rows"));
}

#[test]
fn rejects_unknown_keys_in_every_strict_request_object() {
    let mutations = [
        ("run_request", "run_request"),
        ("fixture", "fixture"),
        ("implementation", "implementation"),
        ("adapter", "adapter"),
        ("environment", "environment"),
        ("cache", "cache"),
        ("sort", "sort"),
        ("filter", "filter"),
        ("group", "group"),
        ("search", "search"),
        ("clock", "clock"),
    ];

    for (object, marker) in mutations {
        let source = match object {
            "run_request" => replace_once(
                &request_text(),
                "\"type\":\"run_request\",",
                "\"type\":\"run_request\",\"extra\":true,",
            ),
            "fixture" => replace_once(
                &request_text(),
                "\"fixture\":{\"id\":",
                "\"fixture\":{\"extra\":true,\"id\":",
            ),
            "implementation" => replace_once(
                &request_text(),
                "\"implementation\":{\"id\":",
                "\"implementation\":{\"extra\":true,\"id\":",
            ),
            "adapter" => replace_once(
                &request_text(),
                "\"adapter\":{\"id\":",
                "\"adapter\":{\"extra\":true,\"id\":",
            ),
            "environment" => replace_once(
                &request_text(),
                "\"environment\":{\"machine_profile_id\":",
                "\"environment\":{\"extra\":true,\"machine_profile_id\":",
            ),
            "cache" => replace_once(
                &request_text(),
                "\"cache\":{\"process\":",
                "\"cache\":{\"extra\":true,\"process\":",
            ),
            "sort" => replace_once(
                &request_text(),
                "\"sort\":{\"field\":",
                "\"sort\":{\"extra\":true,\"field\":",
            ),
            "filter" => replace_once(
                &request_text(),
                "\"filter\":{\"kind\":",
                "\"filter\":{\"extra\":true,\"kind\":",
            ),
            "group" => replace_once(
                &request_text(),
                "\"group\":{\"kind\":",
                "\"group\":{\"extra\":true,\"kind\":",
            ),
            "search" => replace_once(
                &request_text(),
                "\"search\":{\"kind\":",
                "\"search\":{\"extra\":true,\"kind\":",
            ),
            "clock" => replace_once(
                &request_text(),
                "\"clock\":{\"kind\":",
                "\"clock\":{\"extra\":true,\"kind\":",
            ),
            _ => unreachable!("mutation marker {marker} is declared above"),
        };
        assert_eq!(
            schema_error(source).code(),
            ErrorCode::InvalidSchema,
            "{object}"
        );
    }
}

#[test]
fn distinguishes_version_type_from_unsupported_version() {
    let numeric = replace_once(
        &request_text(),
        "\"protocol_version\":1",
        "\"protocol_version\":2",
    );
    assert_eq!(
        schema_error(numeric).code(),
        ErrorCode::UnsupportedProtocolVersion
    );

    let string = replace_once(
        &request_text(),
        "\"protocol_version\":1",
        "\"protocol_version\":\"1\"",
    );
    assert_eq!(schema_error(string).code(), ErrorCode::InvalidSchema);
}

#[test]
fn rejects_scalar_ranges_identifiers_digests_tags_and_field_order() {
    let cases = [
        (
            "\"viewport_size\":40",
            "\"viewport_size\":0",
            ErrorCode::InvalidSchema,
        ),
        (
            "\"run_id\":\"run-local-001\"",
            "\"run_id\":\"bad id\"",
            ErrorCode::InvalidSchema,
        ),
        ("sha256:b684", "sha256:B684", ErrorCode::InvalidSchema),
        (
            "\"group\":{\"kind\":\"none\"}",
            "\"group\":{\"kind\":\"future\"}",
            ErrorCode::InvalidSchema,
        ),
        (
            "[\"identity\",\"kind\"]",
            "[\"kind\",\"identity\"]",
            ErrorCode::InvalidSchema,
        ),
        (
            "\"filter\":{\"kind\":\"none\"}",
            "\"filter\":{\"kind\":\"name_contains\",\"value\":\"file-0001\",\"case_sensitive\":false}",
            ErrorCode::InvalidSchema,
        ),
    ];

    for (from, to, code) in cases {
        let source = replace_once(&request_text(), from, to);
        assert_eq!(schema_error(source).code(), code, "mutation {from}");
    }
}

#[test]
fn rejects_unknown_phase_and_malformed_row_with_distinct_codes() {
    let event = String::from_utf8(golden("invalid_page_event.ndjson")).expect("event is UTF-8");
    let unknown_phase = replace_once(
        &event,
        "\"phase\":\"page.committed\"",
        "\"phase\":\"future.phase\"",
    );
    let phase_error = parse_event_lines(unknown_phase.as_bytes()).expect_err("phase must fail");
    assert_eq!(phase_error.code(), ErrorCode::InvalidPhase);

    let malformed_row = replace_once(&event, "\"kind\":\"directory\"", "\"kind\":\"link\"");
    let row_error = parse_event_lines(malformed_row.as_bytes()).expect_err("row must fail");
    assert_eq!(row_error.code(), ErrorCode::InvalidRow);
}

#[test]
fn unavailable_reasons_are_closed_and_metric_shapes_are_exact() {
    let trace = String::from_utf8(golden("not_supported_trace.ndjson")).expect("trace is UTF-8");
    let bad_reason = replace_once(&trace, "\"not_observable\"", "\"unknown\"");
    assert_eq!(
        parse_event_lines(bad_reason.as_bytes()).unwrap_err().code(),
        ErrorCode::InvalidSchema
    );

    let bad_metric = replace_once(
        &trace,
        "\"unavailable\":\"not_observable\"",
        "\"unavailable\":1",
    );
    assert_eq!(
        parse_event_lines(bad_metric.as_bytes()).unwrap_err().code(),
        ErrorCode::InvalidSchema
    );
}

#[test]
fn rejects_duplicate_and_missing_keys_in_strict_objects() {
    let source = request_text();
    let duplicate_cases = [
        (
            "request",
            "\"protocol_version\":1,",
            "\"protocol_version\":1,\"protocol_version\":1,",
        ),
        (
            "fixture",
            "\"fixture\":{\"id\":",
            "\"fixture\":{\"id\":\"flat-10k-v1\",\"id\":",
        ),
        (
            "implementation",
            "\"implementation\":{\"id\":",
            "\"implementation\":{\"id\":\"filer-core\",\"id\":",
        ),
        (
            "adapter",
            "\"adapter\":{\"id\":",
            "\"adapter\":{\"id\":\"filer-public\",\"id\":",
        ),
        (
            "environment",
            "\"environment\":{\"machine_profile_id\":",
            "\"environment\":{\"machine_profile_id\":\"linux-x86_64-lab-01\",\"machine_profile_id\":",
        ),
        (
            "cache",
            "\"cache\":{\"process\":",
            "\"cache\":{\"process\":\"cold\",\"process\":",
        ),
        (
            "sort",
            "\"sort\":{\"field\":",
            "\"sort\":{\"field\":\"provider_order\",\"field\":",
        ),
        (
            "filter",
            "\"filter\":{\"kind\":",
            "\"filter\":{\"kind\":\"none\",\"kind\":",
        ),
        (
            "group",
            "\"group\":{\"kind\":",
            "\"group\":{\"kind\":\"none\",\"kind\":",
        ),
        (
            "search",
            "\"search\":{\"kind\":",
            "\"search\":{\"kind\":\"none\",\"kind\":",
        ),
        (
            "clock",
            "\"clock\":{\"kind\":",
            "\"clock\":{\"kind\":\"process_monotonic\",\"kind\":",
        ),
    ];

    for (name, from, to) in duplicate_cases {
        let mutated = replace_once(&source, from, to);
        assert_eq!(
            schema_error(mutated).code(),
            ErrorCode::InvalidSchema,
            "duplicate {name}"
        );
    }

    let missing = replace_once(&source, "\"type\":\"run_request\",", "");
    assert_eq!(schema_error(missing).code(), ErrorCode::InvalidSchema);
}
