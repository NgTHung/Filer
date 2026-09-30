//! Shared public-interface trace builders for benchmark validator tests.

use std::path::PathBuf;

pub(super) use filer_core_benchmarks::{
    CanonicalRow, DeclaredCapabilities, ErrorCode, Field, GateResult, Kind, RunValidator,
    ValidatedManifest, canonical_digest,
};
pub(super) use serde_json::{Map, Value, json};

pub(super) fn manifest(name: &str) -> ValidatedManifest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join(name);
    ValidatedManifest::load(path).expect("manifest should validate")
}

pub(super) fn request(
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

pub(super) fn capabilities(
    scenario: &str,
    streaming: bool,
    examined_observable: bool,
) -> DeclaredCapabilities {
    DeclaredCapabilities::new([scenario], streaming, examined_observable)
}

pub(super) struct TraceBuilder {
    fields: Vec<Field>,
    sequence: u64,
    timestamp: u64,
    pub(super) events: Vec<Value>,
    totals: [u64; 4],
}

impl TraceBuilder {
    pub(super) fn new(fields: &[Field]) -> Self {
        Self {
            fields: fields.to_vec(),
            sequence: 0,
            timestamp: 1,
            events: Vec::new(),
            totals: [0; 4],
        }
    }

    pub(super) fn start_sample(&mut self) {
        self.push("sample.started", None, [0; 4], &[], None, None);
    }

    pub(super) fn start_action(&mut self, action: &str) {
        self.push("action.started", Some(action), [0; 4], &[], None, None);
    }

    pub(super) fn milestone(
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

    pub(super) fn end_action(&mut self, action: &str, counts: [u64; 4]) {
        self.push("action.completed", Some(action), counts, &[], None, None);
        for (total, count) in self.totals.iter_mut().zip(counts) {
            *total += count;
        }
    }

    pub(super) fn finish_success(&mut self) {
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

    pub(super) fn finish_not_supported(&mut self, scenario: &str) {
        self.finish_status(
            "not_supported",
            "scenario_not_supported",
            &format!("{scenario} is not supported"),
        );
    }

    pub(super) fn finish_status(&mut self, kind: &str, code: &str, message: &str) {
        let status = json!({"kind":kind,"code":code,"message":message});
        self.push("sample.completed", None, [0; 4], &[], None, Some(status));
    }

    pub(super) fn lines(&self) -> Vec<Vec<u8>> {
        self.events
            .iter()
            .map(|event| {
                let mut bytes = serde_json::to_vec(event).expect("event should serialize");
                bytes.push(b'\n');
                bytes
            })
            .collect()
    }

    pub(super) fn push(
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

    pub(super) fn wire_row(&self, row: &CanonicalRow) -> Value {
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

pub(super) fn validate_trace(
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

pub(super) fn fast_trace(
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

pub(super) fn continuation_trace(manifest: &ValidatedManifest, journey: bool) -> TraceBuilder {
    continuation_trace_order(manifest, journey, false)
}

pub(super) fn continuation_trace_order(
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

pub(super) fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).expect("event should serialize");
    bytes.push(b'\n');
    bytes
}

pub(super) fn resequence(lines: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
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

pub(super) fn run_lines(
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

pub(super) fn sort_trace(manifest: &ValidatedManifest) -> TraceBuilder {
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

pub(super) fn filter_trace(manifest: &ValidatedManifest) -> TraceBuilder {
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

pub(super) fn refresh_trace(manifest: &ValidatedManifest) -> TraceBuilder {
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

pub(super) fn fast_trace_with_examined(
    manifest: &ValidatedManifest,
    examined: Value,
) -> TraceBuilder {
    let mut trace = fast_trace(manifest, "browse.fast.first", false, true);
    let page = trace
        .events
        .iter_mut()
        .find(|event| event["phase"] == "page.committed")
        .expect("page event");
    page["counts"]["examined"] = examined;
    trace
}

pub(super) fn start_error(
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
