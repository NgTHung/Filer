use super::support::*;

#[test]
fn rejects_invalid_scenario_configuration() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.unknown",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    assert_eq!(
        start_error(&manifest, &request, "browse.unknown").code(),
        ErrorCode::InvalidScenarioConfiguration
    );
}

#[test]
fn rejects_fixture_reference_mismatch() {
    let manifest = manifest("flat-10k-v1.json");
    let mut value: Value = serde_json::from_slice(&request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    ))
    .expect("request JSON");
    value["fixture"]["digest"] =
        json!("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    assert_eq!(
        start_error(&manifest, &line(value), "browse.fast.first").code(),
        ErrorCode::FixtureReferenceMismatch
    );
}

#[test]
fn rejects_reused_sample_identity() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut validator = RunValidator::new(
        manifest,
        capabilities("browse.fast.first", false, false),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    let _sample = validator
        .start_sample(&request)
        .expect("first sample should be accepted");
    let error = match validator.start_sample(&request) {
        Ok(_) => panic!("duplicate sample should be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), ErrorCode::DuplicateSample);
}

#[test]
fn rejects_event_correlation_mismatch() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let mut event: Value = serde_json::from_slice(&lines[1]).expect("event JSON");
    event["run_id"] = json!("other-run");
    lines[1] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::CorrelationMismatch
    );
}

#[test]
fn rejects_sequence_gap_or_duplicate() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let mut event: Value = serde_json::from_slice(&lines[1]).expect("event JSON");
    event["sequence"] = json!(9);
    lines[1] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::InvalidSequence
    );
}

#[test]
fn rejects_monotonic_clock_regression() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let mut event: Value = serde_json::from_slice(&lines[1]).expect("event JSON");
    event["timestamp_ns"] = json!(0);
    lines[1] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::ClockRegression
    );
}

#[test]
fn rejects_unknown_or_misordered_phase() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let mut event: Value = serde_json::from_slice(&lines[2]).expect("event JSON");
    event["phase"] = json!("unknown.phase");
    lines[2] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::InvalidPhase
    );
}

#[test]
fn rejects_stale_or_unknown_action() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let mut event: Value = serde_json::from_slice(&lines[2]).expect("event JSON");
    event["action_id"] = json!("completed-action");
    lines[2] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::InvalidAction
    );
}

#[test]
fn rejects_inconsistent_counts() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let page = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("page.committed"))
        .expect("page event");
    let mut event: Value = serde_json::from_slice(&lines[page]).expect("event JSON");
    event["counts"]["emitted"] = json!(255);
    lines[page] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::InvalidCounts
    );
}

#[test]
fn rejects_invalid_row_projection() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let page = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("page.committed"))
        .expect("page event");
    let mut event: Value = serde_json::from_slice(&lines[page]).expect("event JSON");
    event["rows"][0]
        .as_object_mut()
        .expect("row object")
        .remove("kind");
    lines[page] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::InvalidRow
    );
}

#[test]
fn rejects_duplicate_rows() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let page = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("page.committed"))
        .expect("page event");
    let mut event: Value = serde_json::from_slice(&lines[page]).expect("event JSON");
    event["rows"][1] = event["rows"][0].clone();
    lines[page] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::DuplicateIdentity
    );
}

#[test]
fn rejects_output_row_count_mismatch() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let page = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("page.committed"))
        .expect("page event");
    let mut event: Value = serde_json::from_slice(&lines[page]).expect("event JSON");
    event["rows"].as_array_mut().expect("rows array").pop();
    lines[page] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::OutputRowCountMismatch
    );
}

#[test]
fn rejects_wrong_output_digest() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let page = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("page.committed"))
        .expect("page event");
    let mut event: Value = serde_json::from_slice(&lines[page]).expect("event JSON");
    event["output"]["digest"] =
        json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    lines[page] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::OutputDigestMismatch
    );
}

#[test]
fn rejects_incomplete_membership() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let listing = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("listing.completed"))
        .expect("listing event");
    let mut event: Value = serde_json::from_slice(&lines[listing]).expect("event JSON");
    event["rows"][0]["identity"] = json!("outside-manifest");
    lines[listing] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::MembershipMismatch
    );
}

#[test]
fn rejects_missing_required_phase() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let listing = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("listing.completed"))
        .expect("listing event");
    lines.remove(listing);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", resequence(lines),).code(),
        ErrorCode::MissingRequiredPhase
    );
}

#[test]
fn rejects_duplicate_required_phase() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let viewport = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("viewport.committed"))
        .expect("viewport event");
    let mut duplicate = lines.clone();
    duplicate.insert(viewport, lines[viewport].clone());
    assert_eq!(
        run_lines(
            &manifest,
            &request,
            "browse.fast.first",
            resequence(duplicate),
        )
        .code(),
        ErrorCode::DuplicatePhase
    );
}

#[test]
fn rejects_invalid_terminal_status() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    lines.pop();
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", lines).code(),
        ErrorCode::InvalidStatus
    );
}

#[test]
fn rejects_duplicate_terminal_phase() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut lines = fast_trace(&manifest, "browse.fast.first", false, true).lines();
    let terminal = lines.last().cloned().expect("terminal event");
    lines.push(terminal);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", resequence(lines),).code(),
        ErrorCode::DuplicatePhase
    );
}

#[test]
fn rejects_unsupported_scenario_reported_as_success() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.refresh",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("warm", "warm", "reused"),
    );
    let mut trace = TraceBuilder::new(&[Field::Identity, Field::Kind]);
    trace.start_sample();
    trace.finish_success();
    let mut validator = RunValidator::new(
        manifest,
        DeclaredCapabilities::new(Vec::<String>::new(), false, false),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    let mut sample = validator
        .start_sample(&request)
        .expect("request should be accepted");
    sample
        .ingest_line(&trace.lines()[0])
        .expect("sample start should be valid");
    assert_eq!(
        sample
            .ingest_line(&trace.lines()[1])
            .expect_err("unsupported success should be rejected")
            .code(),
        ErrorCode::UnsupportedReportedAsSuccess
    );
}

#[test]
fn rejects_missing_requested_metric() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let trace_manifest = manifest.clone();
    let mut validator = RunValidator::new(
        manifest,
        capabilities("browse.fast.first", false, false),
        ["cpu_time_ns".to_string()],
    )
    .expect("context should be valid");
    let mut sample = validator
        .start_sample(&request)
        .expect("request should be accepted");
    let trace = fast_trace(&trace_manifest, "browse.fast.first", false, true);
    for line in trace.lines() {
        if let Err(error) = sample.ingest_line(&line) {
            assert_eq!(error.code(), ErrorCode::InvalidSchema);
            return;
        }
    }
    panic!("missing requested metric should reject");
}

#[test]
fn accepts_unavailable_snapshot_resource_metric() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "view.sort.name",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("warm", "warm", "reused"),
    );
    let mut trace = sort_trace(&manifest);
    for event in &mut trace.events {
        if matches!(
            event["phase"].as_str(),
            Some(
                "transform.completed" | "view.committed" | "action.completed" | "sample.completed"
            )
        ) {
            event["counts"]["examined"] = json!({"unavailable":"not_observable"});
        }
    }
    let sample = validate_trace(manifest, request, "view.sort.name", trace, false, false);
    assert_eq!(
        sample.status.kind,
        filer_core_benchmarks::StatusKind::Success
    );
}
