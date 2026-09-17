use std::path::PathBuf;

use filer_core_benchmarks::{
    CanonicalRow, DeclaredCapabilities, ErrorCode, Field, GateResult, Kind, RunValidator,
    ValidatedManifest, canonical_digest,
};
use serde_json::{Map, Value, json};

fn manifest(name: &str) -> ValidatedManifest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join(name);
    ValidatedManifest::load(path).expect("manifest should validate")
}

fn request(
    manifest: &ValidatedManifest,
    scenario: &str,
    fields: &[&str],
    sort: (&str, &str),
    filter: Value,
    cache: (&str, &str, &str),
) -> Vec<u8> {
    let reference = manifest.fixture_reference();
    let value = json!({
        "protocol_version": 1,
        "type": "run_request",
        "run_id": "run-local-001",
        "sample_id": "sample-0001",
        "process_id": "process-0001",
        "order_id": "round-01-position-02",
        "scenario_id": scenario,
        "fixture": {"id": reference.id, "digest": reference.digest},
        "implementation": {
            "id": "filer-core",
            "version": "0.3.1",
            "source_revision": "0123456789abcdef0123456789abcdef01234567",
            "build_profile": "release",
            "binary_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
        },
        "adapter": {
            "id": "filer-public",
            "version": "1.0.0",
            "binary_digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
        },
        "environment": {
            "machine_profile_id": "linux-x86_64-lab-01",
            "machine_profile_digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "filesystem_profile_id": "ext4-lab-01",
            "filesystem_profile_digest": "sha256:3333333333333333333333333333333333333333333333333333333333333333"
        },
        "cache": {"process": cache.0, "filesystem": cache.1, "semantic": cache.2},
        "viewport_size": 40,
        "page_size": 256,
        "requested_fields": fields,
        "sort": {"field": sort.0, "direction": sort.1},
        "filter": filter,
        "group": {"kind": "none"},
        "search": {"kind": "none"},
        "clock": {"kind": "process_monotonic", "unit": "nanosecond"}
    });
    let mut bytes = serde_json::to_vec(&value).expect("request should serialize");
    bytes.push(b'\n');
    bytes
}

fn capabilities(
    scenario: &str,
    streaming: bool,
    examined_observable: bool,
) -> DeclaredCapabilities {
    DeclaredCapabilities::new([scenario], streaming, examined_observable)
}

struct TraceBuilder {
    fields: Vec<Field>,
    sequence: u64,
    timestamp: u64,
    events: Vec<Value>,
    totals: [u64; 4],
}

impl TraceBuilder {
    fn new(fields: &[Field]) -> Self {
        Self {
            fields: fields.to_vec(),
            sequence: 0,
            timestamp: 1,
            events: Vec::new(),
            totals: [0; 4],
        }
    }

    fn start_sample(&mut self) {
        self.push("sample.started", None, [0; 4], &[], None, None);
    }

    fn start_action(&mut self, action: &str) {
        self.push("action.started", Some(action), [0; 4], &[], None, None);
    }

    fn milestone(
        &mut self,
        phase: &str,
        action: &str,
        counts: [u64; 4],
        rows: &[CanonicalRow],
        scope: &str,
        continuation: &str,
    ) {
        self.push(
            phase,
            Some(action),
            counts,
            rows,
            Some((scope, continuation)),
            None,
        );
    }

    fn end_action(&mut self, action: &str, counts: [u64; 4]) {
        self.push("action.completed", Some(action), counts, &[], None, None);
        for (total, count) in self.totals.iter_mut().zip(counts) {
            *total += count;
        }
    }

    fn finish_success(&mut self) {
        let status = json!({"kind":"success","code":null,"message":null});
        self.push(
            "sample.completed",
            None,
            self.totals,
            &[],
            None,
            Some(status),
        );
    }

    fn finish_not_supported(&mut self, scenario: &str) {
        self.finish_status(
            "not_supported",
            "scenario_not_supported",
            &format!("{scenario} is not supported"),
        );
    }

    fn finish_status(&mut self, kind: &str, code: &str, message: &str) {
        let status = json!({"kind":kind,"code":code,"message":message});
        self.push("sample.completed", None, [0; 4], &[], None, Some(status));
    }

    fn lines(&self) -> Vec<Vec<u8>> {
        self.events
            .iter()
            .map(|event| {
                let mut bytes = serde_json::to_vec(event).expect("event should serialize");
                bytes.push(b'\n');
                bytes
            })
            .collect()
    }

    fn push(
        &mut self,
        phase: &str,
        action: Option<&str>,
        counts: [u64; 4],
        rows: &[CanonicalRow],
        output: Option<(&str, &str)>,
        status: Option<Value>,
    ) {
        let mut event = Map::new();
        event.insert("protocol_version".to_string(), json!(1));
        event.insert("type".to_string(), json!("run_event"));
        event.insert("run_id".to_string(), json!("run-local-001"));
        event.insert("sample_id".to_string(), json!("sample-0001"));
        event.insert("process_id".to_string(), json!("process-0001"));
        event.insert("order_id".to_string(), json!("round-01-position-02"));
        event.insert("sequence".to_string(), json!(self.sequence));
        event.insert("timestamp_ns".to_string(), json!(self.timestamp));
        event.insert("phase".to_string(), json!(phase));
        event.insert(
            "action_id".to_string(),
            action.map_or(Value::Null, |value| json!(value)),
        );
        event.insert(
            "counts".to_string(),
            json!({"examined":counts[0],"accepted":counts[1],"emitted":counts[2],"visible":counts[3]}),
        );
        event.insert(
            "rows".to_string(),
            Value::Array(rows.iter().map(|row| self.wire_row(row)).collect()),
        );
        let output_value = output.map_or(Value::Null, |(scope, continuation)| {
            let fields = match scope {
                "membership" => vec![Field::Identity],
                "metadata" => vec![
                    Field::Identity,
                    Field::Kind,
                    Field::SizeBytes,
                    Field::ModifiedUnixNs,
                ],
                _ => self.fields.clone(),
            };
            json!({
                "scope": scope,
                "digest": canonical_digest(scope, &fields, rows),
                "row_count": rows.len(),
                "continuation": continuation
            })
        });
        event.insert("output".to_string(), output_value);
        event.insert("metrics".to_string(), json!({}));
        event.insert("status".to_string(), status.unwrap_or(Value::Null));
        self.events.push(Value::Object(event));
        self.sequence += 1;
        self.timestamp += 1;
    }

    fn wire_row(&self, row: &CanonicalRow) -> Value {
        let mut value = Map::new();
        value.insert("identity".to_string(), json!(row.identity));
        value.insert(
            "kind".to_string(),
            json!(match row.kind {
                Kind::File => "file",
                Kind::Directory => "directory",
            }),
        );
        if self.fields.contains(&Field::SizeBytes) {
            value.insert(
                "size_bytes".to_string(),
                row.size_bytes.map_or(Value::Null, |size| json!(size)),
            );
        }
        if self.fields.contains(&Field::ModifiedUnixNs) {
            value.insert("modified_unix_ns".to_string(), json!(row.modified_unix_ns));
        }
        Value::Object(value)
    }
}

fn validate_trace(
    manifest: ValidatedManifest,
    request: Vec<u8>,
    scenario: &str,
    trace: TraceBuilder,
    streaming: bool,
    examined_observable: bool,
) -> filer_core_benchmarks::ValidatedSample {
    let mut validator = RunValidator::new(
        manifest,
        capabilities(scenario, streaming, examined_observable),
        Vec::<String>::new(),
    )
    .expect("validator context should be valid");
    let mut sample = validator
        .start_sample(&request)
        .expect("request should be accepted");
    for line in trace.lines() {
        sample.ingest_line(&line).expect("trace should be accepted");
    }
    sample.finish().expect("EOF should finalize the sample")
}

fn fast_trace(
    manifest: &ValidatedManifest,
    scenario: &str,
    metadata: bool,
    row_first: bool,
) -> TraceBuilder {
    let fields = if metadata {
        vec![
            Field::Identity,
            Field::Kind,
            Field::SizeBytes,
            Field::ModifiedUnixNs,
        ]
    } else {
        vec![Field::Identity, Field::Kind]
    };
    let rows = manifest.expected_rows();
    let mut trace = TraceBuilder::new(&fields);
    trace.start_sample();
    trace.start_action("open");
    if row_first {
        trace.milestone(
            "row.first",
            "open",
            [1; 4],
            &rows[..1],
            "viewport",
            "not_applicable",
        );
    }
    trace.milestone(
        "viewport.committed",
        "open",
        [40; 4],
        &rows[..40],
        "viewport",
        "not_applicable",
    );
    trace.milestone(
        "page.committed",
        "open",
        [256; 4],
        &rows[..256],
        "page",
        "more",
    );
    let listing_counts = [manifest.entry_count() as u64; 4];
    let scope = if metadata { "metadata" } else { "membership" };
    trace.milestone(
        "listing.completed",
        "open",
        listing_counts,
        &rows,
        scope,
        "not_applicable",
    );
    trace.end_action("open", listing_counts);
    trace.finish_success();
    let _ = scenario;
    trace
}

fn continuation_trace(manifest: &ValidatedManifest, journey: bool) -> TraceBuilder {
    continuation_trace_order(manifest, journey, false)
}

fn continuation_trace_order(
    manifest: &ValidatedManifest,
    journey: bool,
    reverse_provider_order: bool,
) -> TraceBuilder {
    let fields = vec![Field::Identity, Field::Kind];
    let mut rows = manifest.expected_rows();
    if reverse_provider_order {
        rows.reverse();
    }
    let mut trace = TraceBuilder::new(&fields);
    trace.start_sample();
    trace.start_action("open");
    trace.milestone(
        "viewport.committed",
        "open",
        [40; 4],
        &rows[..40],
        "viewport",
        "not_applicable",
    );
    trace.milestone(
        "page.committed",
        "open",
        [256; 4],
        &rows[..256],
        "page",
        "more",
    );
    trace.end_action("open", [256; 4]);
    for page in 2..=40 {
        let start = (page - 1) * 256;
        let end = (start + 256).min(rows.len());
        let page_rows = &rows[start..end];
        let count = page_rows.len() as u64;
        let action = format!("page-{page:04}");
        trace.start_action(&action);
        let continuation = if page == 40 { "end" } else { "more" };
        trace.milestone(
            "page.committed",
            &action,
            [count; 4],
            page_rows,
            "page",
            continuation,
        );
        if page == 40 {
            trace.milestone(
                "listing.completed",
                &action,
                [count; 4],
                &rows,
                "membership",
                "not_applicable",
            );
        }
        trace.end_action(&action, [count; 4]);
    }
    if journey {
        let named = manifest.expected_name_rows();
        trace.start_action("sort-name");
        let total = [rows.len() as u64; 4];
        trace.milestone(
            "transform.completed",
            "sort-name",
            total,
            &named,
            "ordered",
            "not_applicable",
        );
        trace.milestone(
            "view.committed",
            "sort-name",
            total,
            &named[..40],
            "viewport",
            "not_applicable",
        );
        trace.end_action("sort-name", total);

        let filtered = manifest
            .expected_filter_rows()
            .expect("flat-10k has filter values");
        trace.start_action("filter-name");
        let filtered_counts = [
            rows.len() as u64,
            filtered.len() as u64,
            filtered.len() as u64,
            filtered.len() as u64,
        ];
        trace.milestone(
            "transform.completed",
            "filter-name",
            filtered_counts,
            &filtered,
            "ordered",
            "not_applicable",
        );
        trace.milestone(
            "view.committed",
            "filter-name",
            filtered_counts,
            &filtered[..40],
            "viewport",
            "not_applicable",
        );
        trace.end_action("filter-name", filtered_counts);

        trace.start_action("clear-filter");
        trace.milestone(
            "transform.completed",
            "clear-filter",
            total,
            &named,
            "ordered",
            "not_applicable",
        );
        trace.milestone(
            "view.committed",
            "clear-filter",
            total,
            &named[..40],
            "viewport",
            "not_applicable",
        );
        trace.end_action("clear-filter", total);

        trace.start_action("refresh");
        trace.milestone(
            "listing.completed",
            "refresh",
            total,
            &rows,
            "membership",
            "not_applicable",
        );
        trace.milestone(
            "transform.completed",
            "refresh",
            total,
            &named,
            "ordered",
            "not_applicable",
        );
        trace.milestone(
            "view.committed",
            "refresh",
            total,
            &named[..40],
            "viewport",
            "not_applicable",
        );
        trace.end_action("refresh", total);
    }
    trace.finish_success();
    trace
}

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
    let mut missing_phase = valid.clone();
    missing_phase.remove(viewport_index);
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

fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).expect("event should serialize");
    bytes.push(b'\n');
    bytes
}

fn resequence(lines: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(sequence, source)| {
            let mut event: Value = serde_json::from_slice(&source).expect("event JSON");
            event["sequence"] = json!(sequence);
            event["timestamp_ns"] = json!(sequence + 1);
            line(event)
        })
        .collect()
}

fn run_lines(
    manifest: &ValidatedManifest,
    request: &[u8],
    scenario: &str,
    lines: Vec<Vec<u8>>,
) -> filer_core_benchmarks::ProtocolError {
    let mut validator = RunValidator::new(
        manifest.clone(),
        capabilities(scenario, true, true),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    let mut sample = validator
        .start_sample(request)
        .expect("request should be accepted");
    for line in lines {
        if let Err(error) = sample.ingest_line(&line) {
            return error;
        }
    }
    sample.finish().expect_err("mutated trace should fail")
}

fn sort_trace(manifest: &ValidatedManifest) -> TraceBuilder {
    let fields = vec![Field::Identity, Field::Kind];
    let rows = manifest.expected_name_rows();
    let mut trace = TraceBuilder::new(&fields);
    trace.start_sample();
    trace.start_action("sort-name");
    let total = [manifest.entry_count() as u64; 4];
    trace.milestone(
        "transform.completed",
        "sort-name",
        total,
        &rows,
        "ordered",
        "not_applicable",
    );
    trace.milestone(
        "view.committed",
        "sort-name",
        total,
        &rows[..40],
        "viewport",
        "not_applicable",
    );
    trace.end_action("sort-name", total);
    trace.finish_success();
    trace
}

fn filter_trace(manifest: &ValidatedManifest) -> TraceBuilder {
    let fields = vec![Field::Identity, Field::Kind];
    let rows = manifest.expected_filter_rows().expect("filter values");
    let mut trace = TraceBuilder::new(&fields);
    trace.start_sample();
    trace.start_action("filter-name");
    let counts = [
        manifest.entry_count() as u64,
        rows.len() as u64,
        rows.len() as u64,
        rows.len() as u64,
    ];
    trace.milestone(
        "transform.completed",
        "filter-name",
        counts,
        &rows,
        "ordered",
        "not_applicable",
    );
    trace.milestone(
        "view.committed",
        "filter-name",
        counts,
        &rows[..40],
        "viewport",
        "not_applicable",
    );
    trace.end_action("filter-name", counts);
    trace.finish_success();
    trace
}

fn refresh_trace(manifest: &ValidatedManifest) -> TraceBuilder {
    let fields = vec![Field::Identity, Field::Kind];
    let provider_rows = manifest.expected_rows();
    let named_rows = manifest.expected_name_rows();
    let mut trace = TraceBuilder::new(&fields);
    trace.start_sample();
    trace.start_action("refresh");
    let total = [manifest.entry_count() as u64; 4];
    trace.milestone(
        "listing.completed",
        "refresh",
        total,
        &provider_rows,
        "membership",
        "not_applicable",
    );
    trace.milestone(
        "transform.completed",
        "refresh",
        total,
        &named_rows,
        "ordered",
        "not_applicable",
    );
    trace.milestone(
        "view.committed",
        "refresh",
        total,
        &named_rows[..40],
        "viewport",
        "not_applicable",
    );
    trace.end_action("refresh", total);
    trace.finish_success();
    trace
}

fn fast_trace_with_examined(manifest: &ValidatedManifest, examined: Value) -> TraceBuilder {
    let mut trace = fast_trace(manifest, "browse.fast.first", false, true);
    let page = trace
        .events
        .iter_mut()
        .find(|event| event["phase"] == "page.committed")
        .expect("page event");
    page["counts"]["examined"] = examined;
    trace
}

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
    trace.events[4]["counts"] =
        json!({"examined":256,"accepted":256,"emitted":256,"visible":256});
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

    let error = run_lines(
        &manifest,
        &request,
        scenario,
        resequence(trace.lines()),
    );

    assert_eq!(error.code(), ErrorCode::InvalidPhase);
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

fn start_error(
    manifest: &ValidatedManifest,
    request: &[u8],
    capability_scenario: &str,
) -> filer_core_benchmarks::ProtocolError {
    let mut validator = RunValidator::new(
        manifest.clone(),
        capabilities(capability_scenario, false, false),
        Vec::<String>::new(),
    )
    .expect("context should be valid");
    match validator.start_sample(request) {
        Ok(_) => panic!("request should be rejected"),
        Err(error) => error,
    }
}

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
    let viewport = lines
        .iter()
        .position(|line| String::from_utf8_lossy(line).contains("viewport.committed"))
        .expect("viewport event");
    lines.remove(viewport);
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
