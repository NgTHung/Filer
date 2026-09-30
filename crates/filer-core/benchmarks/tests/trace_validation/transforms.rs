use super::support::*;

#[test]
fn accepts_name_sort_trace() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "view.sort.name",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("warm", "warm", "reused"),
    );
    let sample = validate_trace(
        manifest.clone(),
        request,
        "view.sort.name",
        sort_trace(&manifest),
        false,
        false,
    );
    assert_eq!(
        sample.structural_gates.first_page_examined,
        GateResult::NotApplicable
    );
}

#[test]
fn accepts_name_filter_trace() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "view.filter.common",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"name_contains","value":"file-0001","case_sensitive":true}),
        ("warm", "warm", "reused"),
    );
    let sample = validate_trace(
        manifest.clone(),
        request,
        "view.filter.common",
        filter_trace(&manifest),
        false,
        false,
    );
    assert_eq!(sample.event_count, 6);
    assert_eq!(
        sample.structural_gates.first_page_examined,
        GateResult::NotApplicable
    );
}

#[test]
fn accepts_refresh_trace() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.refresh",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("warm", "warm", "reused"),
    );
    let sample = validate_trace(
        manifest.clone(),
        request,
        "browse.refresh",
        refresh_trace(&manifest),
        false,
        false,
    );
    assert_eq!(
        sample.status.kind,
        filer_core_benchmarks::StatusKind::Success
    );
}

#[test]
fn accepts_reference_journey_trace() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "journey.browse-reference",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("warm", "warm", "empty"),
    );
    let sample = validate_trace(
        manifest.clone(),
        request,
        "journey.browse-reference",
        continuation_trace(&manifest, true),
        true,
        true,
    );
    assert_eq!(
        sample.status.kind,
        filer_core_benchmarks::StatusKind::Success
    );
}

#[test]
fn does_not_apply_streaming_cap_to_sparse_filter() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "view.filter.common",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"name_contains","value":"file-0001","case_sensitive":true}),
        ("warm", "warm", "reused"),
    );
    let sample = validate_trace(
        manifest.clone(),
        request,
        "view.filter.common",
        filter_trace(&manifest),
        true,
        true,
    );
    assert_eq!(
        sample.structural_gates.first_page_examined,
        GateResult::NotApplicable
    );
}

#[test]
fn rejects_misordered_transform_and_view_phases() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "view.sort.name",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("warm", "warm", "reused"),
    );
    let mut lines = sort_trace(&manifest).lines();
    let transform = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("transform.completed"))
        .expect("transform event");
    let view = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("view.committed"))
        .expect("view event");
    lines.swap(transform, view);
    assert_eq!(
        run_lines(&manifest, &request, "view.sort.name", resequence(lines)).code(),
        ErrorCode::InvalidPhase
    );
}

#[test]
fn rejects_skipped_pages_and_output_after_completed_action() {
    let flat_10k = manifest("flat-10k-v1.json");
    let browse_request = request(
        &flat_10k,
        "browse.next",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let skipped = continuation_trace(&flat_10k, false)
        .lines()
        .into_iter()
        .filter(|line| !String::from_utf8_lossy(line).contains("page-0010"))
        .collect::<Vec<_>>();
    assert_eq!(
        run_lines(
            &flat_10k,
            &browse_request,
            "browse.next",
            resequence(skipped),
        )
        .code(),
        ErrorCode::InvalidAction
    );

    let sort_request = request(
        &flat_10k,
        "view.sort.name",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("warm", "warm", "reused"),
    );
    let mut output_after_completion = sort_trace(&flat_10k).lines();
    let transform = output_after_completion
        .iter()
        .find(|line| String::from_utf8_lossy(line).contains("transform.completed"))
        .cloned()
        .expect("transform event");
    let completed = output_after_completion
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("action.completed"))
        .expect("action completion");
    output_after_completion.insert(completed + 1, transform);
    assert_eq!(
        run_lines(
            &flat_10k,
            &sort_request,
            "view.sort.name",
            resequence(output_after_completion),
        )
        .code(),
        ErrorCode::InvalidAction
    );
}

#[test]
fn rejects_unavailable_correctness_count() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "view.filter.common",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"name_contains","value":"file-0001","case_sensitive":true}),
        ("warm", "warm", "reused"),
    );
    let mut lines = filter_trace(&manifest).lines();
    let transform = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("transform.completed"))
        .expect("transform event");
    let mut event: Value = serde_json::from_slice(&lines[transform]).expect("event JSON");
    event["counts"]["accepted"] = json!({"unavailable":"not_observable"});
    lines[transform] = line(event);
    assert_eq!(
        run_lines(&manifest, &request, "view.filter.common", resequence(lines),).code(),
        ErrorCode::RequiredCountUnavailable
    );
}

#[test]
fn rejects_cache_state_outside_scenario_contract() {
    let manifest = manifest("flat-10k-v1.json");
    let bad_request = request(
        &manifest,
        "view.sort.name",
        &["identity", "kind"],
        ("name", "ascending"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    let mut validator = RunValidator::new(
        manifest,
        capabilities("view.sort.name", false, false),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    let error = match validator.start_sample(&bad_request) {
        Ok(_) => panic!("cold empty sort must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), ErrorCode::InvalidScenarioConfiguration);
}

#[test]
fn accepts_error_and_cancelled_diagnostics() {
    let manifest = manifest("flat-10k-v1.json");
    let request = request(
        &manifest,
        "browse.fast.first",
        &["identity", "kind"],
        ("provider_order", "none"),
        json!({"kind":"none"}),
        ("cold", "warm", "empty"),
    );
    for (kind, code) in [
        ("error", "fixture_digest_mismatch"),
        ("cancelled", "cancelled_at_barrier"),
    ] {
        let mut trace = TraceBuilder::new(&[Field::Identity, Field::Kind]);
        trace.start_sample();
        trace.finish_status(kind, code, "diagnostic");
        let sample = validate_trace(
            manifest.clone(),
            request.clone(),
            "browse.fast.first",
            trace,
            true,
            true,
        );
        assert_ne!(
            sample.status.kind,
            filer_core_benchmarks::StatusKind::Success
        );
    }
}
