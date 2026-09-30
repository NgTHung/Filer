use super::support::*;

#[test]
fn accepts_fast_first_scale_metadata_and_continuation_traces() {
    let flat_10k = manifest("flat-10k-v1.json");
    let request_10k = request(
        &flat_10k,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let sample = validate_trace(
        flat_10k.clone(),
        request_10k,
        "browse.fast.first",
        fast_trace(&flat_10k, "browse.fast.first", false, true),
        true,
        true,
    );
    assert_eq!(
        sample.structural_gates.first_page_examined,
        GateResult::Passed
    );

    let flat_100k = manifest("flat-100k-v1.json");
    let request_scale = request(
        &flat_100k,
        "browse.fast.scale",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let sample = validate_trace(
        flat_100k.clone(),
        request_scale,
        "browse.fast.scale",
        fast_trace(&flat_100k, "browse.fast.scale", false, false),
        true,
        true,
    );
    assert_eq!(
        sample.status.kind,
        filer_core_benchmarks::StatusKind::Success
    );

    let request_metadata = request(
        &flat_10k,
        "browse.metadata.first",
        &["identity", "kind", "size_bytes", "modified_unix_ns"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let sample = validate_trace(
        flat_10k.clone(),
        request_metadata,
        "browse.metadata.first",
        fast_trace(&flat_10k, "browse.metadata.first", true, false),
        true,
        true,
    );
    assert_eq!(sample.event_count, 7);

    let request_next = request(
        &flat_10k,
        "browse.next",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let sample = validate_trace(
        flat_10k.clone(),
        request_next,
        "browse.next",
        continuation_trace(&flat_10k, false),
        true,
        true,
    );
    assert_eq!(
        sample.structural_gates.first_page_examined,
        GateResult::Passed
    );

    let request_next_reordered = request(
        &flat_10k,
        "browse.next",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let sample = validate_trace(
        flat_10k.clone(),
        request_next_reordered,
        "browse.next",
        continuation_trace_order(&flat_10k, false, true),
        true,
        true,
    );
    assert_eq!(
        sample.status.kind,
        filer_core_benchmarks::StatusKind::Success
    );
}

#[test]
fn accepts_non_success_without_success_milestones() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut trace = TraceBuilder::new(&[Field::Identity, Field::Kind]);
    trace.start_sample();
    trace.finish_not_supported("browse.fast.first");
    let mut validator = RunValidator::new(
        manifest,
        DeclaredCapabilities::new(Vec::<String>::new(), false, false),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    let mut sample = validator
        .start_sample(&request)
        .expect("request should be accepted");
    for line in trace.lines() {
        sample
            .ingest_line(&line)
            .expect("non-success trace is valid");
    }
    assert_eq!(
        sample.finish().expect("EOF should finalize").status.kind,
        filer_core_benchmarks::StatusKind::NotSupported
    );
}

#[test]
fn rejects_reused_sample_identity_and_unsupported_success_before_milestones() {
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
        manifest.clone(),
        DeclaredCapabilities::new(Vec::<String>::new(), false, false),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    let _sample = validator
        .start_sample(&request)
        .expect("first sample should be accepted");
    let duplicate_error = match validator.start_sample(&request) {
        Ok(_) => panic!("duplicate sample should be rejected"),
        Err(error) => error,
    };
    assert_eq!(duplicate_error.code(), ErrorCode::DuplicateSample);

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
    let error = sample
        .ingest_line(&trace.lines()[1])
        .expect_err("success must be rejected");
    assert_eq!(error.code(), ErrorCode::UnsupportedReportedAsSuccess);
}

#[test]
fn rejects_missing_rows_duplicates_digests_sequence_clock_and_stale_actions() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let valid = fast_trace(&manifest, "browse.fast.first", false, true).lines();

    let page_index = valid
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("page.committed"))
        .expect("page event");
    let viewport_index = valid
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("viewport.committed"))
        .expect("viewport event");

    let listing_index = valid
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("listing.completed"))
        .expect("listing event");
    let mut missing_phase = valid.clone();
    missing_phase.remove(listing_index);
    assert_eq!(
        run_lines(
            &manifest,
            &request,
            "browse.fast.first",
            resequence(missing_phase)
        )
        .code(),
        ErrorCode::MissingRequiredPhase
    );

    let mut duplicate_phase = valid.clone();
    duplicate_phase.insert(page_index, duplicate_phase[viewport_index].clone());
    assert_eq!(
        run_lines(
            &manifest,
            &request,
            "browse.fast.first",
            resequence(duplicate_phase)
        )
        .code(),
        ErrorCode::DuplicatePhase
    );

    let mut invalid_counts = valid.clone();
    let mut count_event: Value =
        serde_json::from_slice(&invalid_counts[page_index]).expect("page JSON");
    count_event["counts"]["emitted"] = json!(255);
    invalid_counts[page_index] = line(count_event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", invalid_counts).code(),
        ErrorCode::InvalidCounts
    );

    let mut missing_rows = valid.clone();
    let mut page: Value = serde_json::from_slice(&missing_rows[page_index]).expect("page JSON");
    page["rows"].as_array_mut().expect("rows array").pop();
    missing_rows[page_index] = line(page);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", missing_rows).code(),
        ErrorCode::OutputRowCountMismatch
    );

    let mut duplicate = valid.clone();
    let mut page: Value = serde_json::from_slice(&duplicate[page_index]).expect("page JSON");
    let first = page["rows"][0].clone();
    page["rows"][1] = first;
    duplicate[page_index] = line(page);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", duplicate).code(),
        ErrorCode::DuplicateIdentity
    );

    let mut digest = valid.clone();
    let mut page: Value = serde_json::from_slice(&digest[page_index]).expect("page JSON");
    page["output"]["digest"] =
        json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    digest[page_index] = line(page);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", digest).code(),
        ErrorCode::OutputDigestMismatch
    );

    let mut sequence = valid.clone();
    let mut event: Value = serde_json::from_slice(&sequence[1]).expect("event JSON");
    event["sequence"] = json!(9);
    sequence[1] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", sequence).code(),
        ErrorCode::InvalidSequence
    );

    let mut clock = valid.clone();
    let mut event: Value = serde_json::from_slice(&clock[1]).expect("event JSON");
    event["timestamp_ns"] = json!(0);
    clock[1] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", clock).code(),
        ErrorCode::ClockRegression
    );

    let mut stale = valid.clone();
    let mut event: Value = serde_json::from_slice(&stale[2]).expect("event JSON");
    event["action_id"] = json!("old-action");
    stale[2] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "browse.fast.first", stale).code(),
        ErrorCode::InvalidAction
    );
}

#[test]
fn accepts_optional_row_first_on_open() {
    let manifest = manifest("flat-100k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.scale",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    validate_trace(
        manifest.clone(),
        request,
        "browse.fast.scale",
        fast_trace(&manifest, "browse.fast.scale", false, true),
        true,
        true,
    );
}

#[test]
fn accepts_page_before_viewport_with_cumulative_counts() {
    let manifest = manifest("flat-10k-v1.json");
    let scenario = "browse.fast.first";
    let request = request(
        &manifest,
        scenario,
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "uncontrolled", "empty"),
    );
    let mut trace = fast_trace(&manifest, scenario, false, true);
    trace.events.swap(3, 4);
    trace.events[4]["counts"] = json!({"examined":256,"accepted":256,"emitted":256,"visible":256});
    for (sequence, event) in trace.events.iter_mut().enumerate() {
        event["sequence"] = json!(sequence);
        event["timestamp_ns"] = json!(sequence + 1);
    }

    let sample = validate_trace(manifest, request, scenario, trace, true, true);

    assert_eq!(
        sample.status.kind,
        filer_core_benchmarks::StatusKind::Success
    );
}

#[test]
fn rejects_listing_before_required_row_first() {
    let manifest = manifest("flat-10k-v1.json");
    let scenario = "browse.fast.first";
    let request = request(
        &manifest,
        scenario,
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "uncontrolled", "empty"),
    );
    let mut trace = fast_trace(&manifest, scenario, false, true);
    let mut listing = trace.events.remove(5);
    listing["counts"] = json!({"examined":0,"accepted":0,"emitted":0,"visible":0});
    trace.events.insert(2, listing);

    let error = run_lines(&manifest, &request, scenario, resequence(trace.lines()));

    assert_eq!(error.code(), ErrorCode::InvalidPhase);
}

#[test]
fn rejects_inflated_continuation_proof_counts() {
    let manifest = manifest("flat-10k-v1.json");
    let scenario = "browse.next";
    let request = request(
        &manifest,
        scenario,
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "uncontrolled", "empty"),
    );
    let mut trace = continuation_trace(&manifest, false);
    for event in &mut trace.events {
        if event["action_id"] == "page-0040"
            && matches!(
                event["phase"].as_str(),
                Some("listing.completed" | "action.completed")
            )
        {
            event["counts"] =
                json!({"examined":10000,"accepted":10000,"emitted":10000,"visible":10000});
        }
        if event["phase"] == "sample.completed" {
            event["counts"] =
                json!({"examined":19984,"accepted":19984,"emitted":19984,"visible":19984});
        }
    }

    let error = run_lines(&manifest, &request, scenario, trace.lines());

    assert_eq!(error.code(), ErrorCode::InvalidCounts);
}

#[test]
fn unavailable_first_page_gate_is_not_replaced_by_later_pages() {
    let manifest = manifest("flat-10k-v1.json");
    let scenario = "browse.next";
    let request = request(
        &manifest,
        scenario,
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "uncontrolled", "empty"),
    );
    let mut trace = continuation_trace(&manifest, false);
    for event in &mut trace.events {
        let first_page_total = event["action_id"] == "open"
            && matches!(
                event["phase"].as_str(),
                Some("page.committed" | "action.completed")
            );
        if first_page_total || event["phase"] == "sample.completed" {
            event["counts"]["examined"] = json!({"unavailable":"not_observable"});
        }
    }

    let sample = validate_trace(manifest, request, scenario, trace, true, true);

    assert_eq!(
        sample.structural_gates.first_page_examined,
        GateResult::NotEvaluable
    );
}

#[test]
fn classifies_streaming_first_page_gate() {
    let flat_10k = manifest("flat-10k-v1.json");
    let request = request(
        &flat_10k,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let passed = validate_trace(
        flat_10k.clone(),
        request.clone(),
        "browse.fast.first",
        fast_trace_with_examined(&flat_10k, json!(512)),
        true,
        true,
    );
    assert_eq!(
        passed.structural_gates.first_page_examined,
        GateResult::Passed
    );
    let failed = validate_trace(
        flat_10k.clone(),
        request.clone(),
        "browse.fast.first",
        fast_trace_with_examined(&flat_10k, json!(513)),
        true,
        true,
    );
    assert_eq!(
        failed.structural_gates.first_page_examined,
        GateResult::Failed
    );
    let unavailable = validate_trace(
        flat_10k.clone(),
        request.clone(),
        "browse.fast.first",
        fast_trace_with_examined(&flat_10k, json!({"unavailable":"not_observable"})),
        true,
        true,
    );
    assert_eq!(
        unavailable.structural_gates.first_page_examined,
        GateResult::NotEvaluable
    );
    let not_streaming = validate_trace(
        flat_10k.clone(),
        request.clone(),
        "browse.fast.first",
        fast_trace_with_examined(&flat_10k, json!(512)),
        false,
        true,
    );
    assert_eq!(
        not_streaming.structural_gates.first_page_examined,
        GateResult::NotApplicable
    );
    let not_observable = validate_trace(
        flat_10k.clone(),
        request,
        "browse.fast.first",
        fast_trace_with_examined(&flat_10k, json!(512)),
        true,
        false,
    );
    assert_eq!(
        not_observable.structural_gates.first_page_examined,
        GateResult::NotEvaluable
    );
}
