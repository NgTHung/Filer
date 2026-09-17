//! # Protocol schema
//!
//! This module parses one strict request or event frame and converts it to
//! typed values. Semantic relationships between a request and its manifest
//! belong to the trace validator, so schema parsing stays reusable.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::{DeserializeOwned, MapAccess, Visitor};

use crate::{ErrorCode, ErrorContext, ProtocolError};

const PROTOCOL_VERSION: u64 = 1;
const DIGEST_PREFIX: &str = "sha256:";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub protocol_version: u64,
    pub message_type: String,
    pub run_id: String,
    pub sample_id: String,
    pub process_id: String,
    pub order_id: String,
    pub scenario_id: String,
    pub fixture: FixtureReference,
    pub implementation: Implementation,
    pub adapter: Adapter,
    pub environment: Environment,
    pub cache: CacheState,
    pub viewport_size: u64,
    pub page_size: u64,
    pub requested_fields: Vec<Field>,
    pub sort: Sort,
    pub filter: Filter,
    pub group: Group,
    pub search: Group,
    pub clock: Clock,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureReference {
    pub id: String,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Implementation {
    pub id: String,
    pub version: String,
    pub source_revision: String,
    pub build_profile: String,
    pub binary_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Adapter {
    pub id: String,
    pub version: String,
    pub binary_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Environment {
    pub machine_profile_id: String,
    pub machine_profile_digest: String,
    pub filesystem_profile_id: String,
    pub filesystem_profile_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheState {
    pub process: ProcessCache,
    pub filesystem: FilesystemCache,
    pub semantic: SemanticCache,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessCache {
    Cold,
    Warm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemCache {
    ControlledCold,
    FreshCopy,
    Warm,
    Uncontrolled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCache {
    Empty,
    Reset,
    Reused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Identity,
    Kind,
    SizeBytes,
    ModifiedUnixNs,
}

impl Field {
    pub const ALL: [Self; 4] = [
        Self::Identity,
        Self::Kind,
        Self::SizeBytes,
        Self::ModifiedUnixNs,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Kind => "kind",
            Self::SizeBytes => "size_bytes",
            Self::ModifiedUnixNs => "modified_unix_ns",
        }
    }

    fn parse(value: &str) -> Result<Self, ProtocolError> {
        match value {
            "identity" => Ok(Self::Identity),
            "kind" => Ok(Self::Kind),
            "size_bytes" => Ok(Self::SizeBytes),
            "modified_unix_ns" => Ok(Self::ModifiedUnixNs),
            _ => Err(schema_error("requested_fields contains an unknown field")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sort {
    ProviderOrder,
    NameAscending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Filter {
    None,
    NameContains { value: String, case_sensitive: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Clock {
    ProcessMonotonicNanosecond,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub protocol_version: u64,
    pub message_type: String,
    pub run_id: String,
    pub sample_id: String,
    pub process_id: String,
    pub order_id: String,
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub phase: Phase,
    pub action_id: Option<String>,
    pub counts: Counts,
    pub rows: Vec<Row>,
    pub output: Option<Output>,
    pub metrics: BTreeMap<String, MetricValue>,
    pub status: Option<Status>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    SampleStarted,
    ActionStarted,
    RowFirst,
    ViewportCommitted,
    PageCommitted,
    ListingCompleted,
    TransformCompleted,
    ViewCommitted,
    ActionCompleted,
    SampleCompleted,
}

impl Phase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SampleStarted => "sample.started",
            Self::ActionStarted => "action.started",
            Self::RowFirst => "row.first",
            Self::ViewportCommitted => "viewport.committed",
            Self::PageCommitted => "page.committed",
            Self::ListingCompleted => "listing.completed",
            Self::TransformCompleted => "transform.completed",
            Self::ViewCommitted => "view.committed",
            Self::ActionCompleted => "action.completed",
            Self::SampleCompleted => "sample.completed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Counts {
    pub examined: MetricValue,
    pub accepted: MetricValue,
    pub emitted: MetricValue,
    pub visible: MetricValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetricValue {
    Observed(u64),
    Unavailable(UnavailableReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailableReason {
    Unsupported,
    PermissionDenied,
    NotObservable,
    PlatformUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub identity: Option<String>,
    pub kind: Option<Kind>,
    pub size_bytes: Option<Option<u64>>,
    pub modified_unix_ns: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    File,
    Directory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Output {
    pub scope: OutputScope,
    pub digest: String,
    pub row_count: u64,
    pub continuation: Continuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputScope {
    Membership,
    Metadata,
    Ordered,
    Page,
    Viewport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Continuation {
    More,
    End,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Status {
    pub kind: StatusKind,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusKind {
    Success,
    NotSupported,
    Error,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    protocol_version: u64,
    #[serde(rename = "type")]
    message_type: String,
    run_id: String,
    sample_id: String,
    process_id: String,
    order_id: String,
    scenario_id: String,
    fixture: RawFixtureReference,
    implementation: RawImplementation,
    adapter: RawAdapter,
    environment: RawEnvironment,
    cache: RawCacheState,
    viewport_size: u64,
    page_size: u64,
    requested_fields: Vec<String>,
    sort: RawSort,
    filter: RawFilter,
    group: RawTag,
    search: RawTag,
    clock: RawClock,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFixtureReference {
    id: String,
    digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawImplementation {
    id: String,
    version: String,
    source_revision: String,
    build_profile: String,
    binary_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAdapter {
    id: String,
    version: String,
    binary_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvironment {
    machine_profile_id: String,
    machine_profile_digest: String,
    filesystem_profile_id: String,
    filesystem_profile_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCacheState {
    process: String,
    filesystem: String,
    semantic: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSort {
    field: String,
    direction: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFilter {
    kind: String,
    value: Option<String>,
    case_sensitive: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTag {
    kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClock {
    kind: String,
    unit: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    protocol_version: u64,
    #[serde(rename = "type")]
    message_type: String,
    run_id: String,
    sample_id: String,
    process_id: String,
    order_id: String,
    sequence: u64,
    timestamp_ns: u64,
    phase: String,
    action_id: Presence<String>,
    counts: RawCounts,
    rows: Vec<serde_json::Value>,
    output: Presence<RawOutput>,
    metrics: StrictMap<RawMetricValue>,
    status: Presence<RawStatus>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCounts {
    examined: RawMetricValue,
    accepted: RawMetricValue,
    emitted: RawMetricValue,
    visible: RawMetricValue,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum RawMetricValue {
    Observed(u64),
    Unavailable(RawUnavailable),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnavailable {
    unavailable: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    #[serde(default)]
    identity: Presence<String>,
    #[serde(default)]
    kind: Presence<String>,
    #[serde(default)]
    size_bytes: Presence<Option<u64>>,
    #[serde(default)]
    modified_unix_ns: Presence<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutput {
    scope: String,
    digest: String,
    row_count: u64,
    continuation: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatus {
    kind: String,
    code: Presence<String>,
    message: Presence<String>,
}

#[derive(Clone, Debug)]
enum Presence<T> {
    Missing,
    Null,
    Value(T),
}

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self::Missing
    }
}

impl<'de, T> Deserialize<'de> for Presence<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PresenceVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for PresenceVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Presence<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a nullable protocol value")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Presence::Null)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Presence::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                T::deserialize(deserializer).map(Presence::Value)
            }
        }

        deserializer.deserialize_option(PresenceVisitor(std::marker::PhantomData))
    }
}

#[derive(Clone, Debug)]
struct StrictMap<V>(BTreeMap<String, V>);

impl<'de, V> Deserialize<'de> for StrictMap<V>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrictMapVisitor<V>(std::marker::PhantomData<V>);

        impl<'de, V> Visitor<'de> for StrictMapVisitor<V>
        where
            V: Deserialize<'de>,
        {
            type Value = StrictMap<V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object with unique keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some(key) = access.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate object key {key}"
                        )));
                    }
                    let value = access.next_value()?;
                    values.insert(key, value);
                }
                Ok(StrictMap(values))
            }
        }

        deserializer.deserialize_map(StrictMapVisitor(std::marker::PhantomData))
    }
}

pub fn parse_request_bytes(bytes: &[u8]) -> Result<Request, ProtocolError> {
    let raw: RawRequest = parse_frame(bytes, FrameKind::Request)?;
    convert_request(raw)
}

pub fn parse_event_line(bytes: &[u8]) -> Result<Event, ProtocolError> {
    let raw: RawEvent = parse_frame(bytes, FrameKind::Event)?;
    convert_event(raw)
}

pub fn parse_event_lines(bytes: &[u8]) -> Result<Vec<Event>, ProtocolError> {
    if std::str::from_utf8(bytes).is_err() {
        return Err(malformed("event stream is not UTF-8"));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(malformed(
            "event stream is truncated or missing its final newline",
        ));
    }
    let mut events = Vec::new();
    for (index, line) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        events.push(parse_event_line(line).map_err(|error| error.with_line(index + 1))?);
    }
    Ok(events)
}

#[derive(Clone, Copy)]
enum FrameKind {
    Request,
    Event,
}

fn parse_frame<T>(bytes: &[u8], kind: FrameKind) -> Result<T, ProtocolError>
where
    T: DeserializeOwned,
{
    if std::str::from_utf8(bytes).is_err() {
        return Err(malformed("frame is not UTF-8"));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(malformed("frame is not newline terminated"));
    }
    let body = &bytes[..bytes.len() - 1];
    let body = body.strip_suffix(b"\r").unwrap_or(body);
    if body.is_empty() || body.iter().any(|byte| *byte == b'\n' || *byte == b'\r') {
        return Err(malformed("frame must contain exactly one JSON object"));
    }
    if !body.iter().any(|byte| !byte.is_ascii_whitespace()) {
        return Err(malformed("frame is empty"));
    }
    let first = body
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .copied();
    if matches!(kind, FrameKind::Event) && first != Some(b'{') {
        return Err(ProtocolError::new(
            ErrorCode::UnexpectedStdout,
            "standard output line is not a JSON event object",
        ));
    }
    serde_json::from_slice(body).map_err(|error| {
        let (code, message) = match error.classify() {
            serde_json::error::Category::Data => (
                ErrorCode::InvalidSchema,
                "JSON object does not match the protocol schema",
            ),
            serde_json::error::Category::Syntax | serde_json::error::Category::Eof => (
                ErrorCode::MalformedJson,
                "frame is not one valid JSON object",
            ),
            serde_json::error::Category::Io => {
                (ErrorCode::MalformedJson, "frame could not be read")
            }
        };
        ProtocolError::new(code, message).with_context(ErrorContext {
            field: Some(error.to_string()),
            ..ErrorContext::default()
        })
    })
}

fn convert_request(raw: RawRequest) -> Result<Request, ProtocolError> {
    if raw.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::new(
            ErrorCode::UnsupportedProtocolVersion,
            "request protocol_version must be 1",
        ));
    }
    if raw.message_type != "run_request" {
        return Err(schema_error("request type must be run_request"));
    }
    for (name, value) in [
        ("run_id", &raw.run_id),
        ("sample_id", &raw.sample_id),
        ("process_id", &raw.process_id),
        ("order_id", &raw.order_id),
        ("scenario_id", &raw.scenario_id),
        ("fixture.id", &raw.fixture.id),
        ("implementation.id", &raw.implementation.id),
        ("implementation.version", &raw.implementation.version),
        (
            "implementation.build_profile",
            &raw.implementation.build_profile,
        ),
        ("adapter.id", &raw.adapter.id),
        ("adapter.version", &raw.adapter.version),
        (
            "environment.machine_profile_id",
            &raw.environment.machine_profile_id,
        ),
        (
            "environment.filesystem_profile_id",
            &raw.environment.filesystem_profile_id,
        ),
    ] {
        validate_identifier(value).map_err(|error| error.with_field(name))?;
    }
    for (name, value) in [
        ("fixture.digest", &raw.fixture.digest),
        (
            "implementation.binary_digest",
            &raw.implementation.binary_digest,
        ),
        ("adapter.binary_digest", &raw.adapter.binary_digest),
        (
            "environment.machine_profile_digest",
            &raw.environment.machine_profile_digest,
        ),
        (
            "environment.filesystem_profile_digest",
            &raw.environment.filesystem_profile_digest,
        ),
    ] {
        validate_digest(value).map_err(|error| error.with_field(name))?;
    }
    if raw.implementation.source_revision.len() != 40
        || !raw
            .implementation
            .source_revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(
            schema_error("implementation.source_revision is not a 40-character hex id")
                .with_field("implementation.source_revision"),
        );
    }
    if raw.viewport_size == 0
        || raw.viewport_size > raw.page_size
        || raw.page_size == 0
        || raw.page_size > 4096
    {
        return Err(schema_error(
            "viewport_size and page_size are outside their allowed ranges",
        ));
    }
    let requested_fields = parse_requested_fields(&raw.requested_fields)?;
    let sort = parse_sort(raw.sort)?;
    let filter = parse_filter(raw.filter)?;
    if raw.group.kind != "none" || raw.search.kind != "none" {
        return Err(schema_error("group and search must use kind none"));
    }
    let cache = parse_cache(raw.cache)?;
    if raw.clock.kind != "process_monotonic" || raw.clock.unit != "nanosecond" {
        return Err(schema_error("clock must be process_monotonic nanosecond"));
    }
    Ok(Request {
        protocol_version: raw.protocol_version,
        message_type: raw.message_type,
        run_id: raw.run_id,
        sample_id: raw.sample_id,
        process_id: raw.process_id,
        order_id: raw.order_id,
        scenario_id: raw.scenario_id,
        fixture: FixtureReference {
            id: raw.fixture.id,
            digest: raw.fixture.digest,
        },
        implementation: Implementation {
            id: raw.implementation.id,
            version: raw.implementation.version,
            source_revision: raw.implementation.source_revision,
            build_profile: raw.implementation.build_profile,
            binary_digest: raw.implementation.binary_digest,
        },
        adapter: Adapter {
            id: raw.adapter.id,
            version: raw.adapter.version,
            binary_digest: raw.adapter.binary_digest,
        },
        environment: Environment {
            machine_profile_id: raw.environment.machine_profile_id,
            machine_profile_digest: raw.environment.machine_profile_digest,
            filesystem_profile_id: raw.environment.filesystem_profile_id,
            filesystem_profile_digest: raw.environment.filesystem_profile_digest,
        },
        cache,
        viewport_size: raw.viewport_size,
        page_size: raw.page_size,
        requested_fields,
        sort,
        filter,
        group: Group::None,
        search: Group::None,
        clock: Clock::ProcessMonotonicNanosecond,
    })
}

fn convert_event(raw: RawEvent) -> Result<Event, ProtocolError> {
    if raw.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::new(
            ErrorCode::UnsupportedProtocolVersion,
            "event protocol_version must be 1",
        ));
    }
    if raw.message_type != "run_event" {
        return Err(ProtocolError::new(
            ErrorCode::UnexpectedStdout,
            "standard output JSON object is not a run_event",
        ));
    }
    for (name, value) in [
        ("run_id", &raw.run_id),
        ("sample_id", &raw.sample_id),
        ("process_id", &raw.process_id),
        ("order_id", &raw.order_id),
    ] {
        validate_identifier(value).map_err(|error| error.with_field(name))?;
    }
    let phase = parse_phase(&raw.phase)?;
    let action_id = required_nullable_string(raw.action_id, "action_id")?;
    let output = required_nullable(raw.output, "output")?
        .map(convert_output)
        .transpose()?;
    let status = required_nullable(raw.status, "status")?
        .map(convert_status)
        .transpose()?;
    let rows = raw
        .rows
        .into_iter()
        .map(convert_row_value)
        .collect::<Result<Vec<_>, _>>()?;
    let metrics = raw
        .metrics
        .0
        .into_iter()
        .map(|(name, value)| {
            validate_identifier(&name)
                .map_err(|error| error.with_field(format!("metrics.{name}")))?;
            Ok((name, convert_metric(value)?))
        })
        .collect::<Result<BTreeMap<_, _>, ProtocolError>>()?;
    Ok(Event {
        protocol_version: raw.protocol_version,
        message_type: raw.message_type,
        run_id: raw.run_id,
        sample_id: raw.sample_id,
        process_id: raw.process_id,
        order_id: raw.order_id,
        sequence: raw.sequence,
        timestamp_ns: raw.timestamp_ns,
        phase,
        action_id,
        counts: convert_counts(raw.counts)?,
        rows,
        output,
        metrics,
        status,
    })
}

fn convert_counts(raw: RawCounts) -> Result<Counts, ProtocolError> {
    Ok(Counts {
        examined: convert_metric(raw.examined)?,
        accepted: convert_metric(raw.accepted)?,
        emitted: convert_metric(raw.emitted)?,
        visible: convert_metric(raw.visible)?,
    })
}

fn convert_metric(raw: RawMetricValue) -> Result<MetricValue, ProtocolError> {
    match raw {
        RawMetricValue::Observed(value) => Ok(MetricValue::Observed(value)),
        RawMetricValue::Unavailable(value) => Ok(MetricValue::Unavailable(parse_unavailable(
            &value.unavailable,
        )?)),
    }
}

fn convert_row_value(value: serde_json::Value) -> Result<Row, ProtocolError> {
    let raw: RawRow = serde_json::from_value(value).map_err(|error| {
        ProtocolError::new(
            ErrorCode::InvalidRow,
            "row does not match the canonical shape",
        )
        .with_context(ErrorContext {
            field: Some(error.to_string()),
            ..ErrorContext::default()
        })
    })?;
    let identity = match raw.identity {
        Presence::Missing => None,
        Presence::Null => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "row identity must be a string",
            ));
        }
        Presence::Value(value) => Some(value),
    };
    let kind = match raw.kind {
        Presence::Missing => None,
        Presence::Null => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "row kind must be a string",
            ));
        }
        Presence::Value(value) => Some(match value.as_str() {
            "file" => Kind::File,
            "directory" => Kind::Directory,
            _ => {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRow,
                    "row kind must be file or directory",
                ));
            }
        }),
    };
    let size_bytes = match raw.size_bytes {
        Presence::Missing => None,
        Presence::Null => Some(None),
        Presence::Value(value) => Some(value),
    };
    let modified_unix_ns = match raw.modified_unix_ns {
        Presence::Missing => None,
        Presence::Null => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "row modified_unix_ns must be an integer",
            ));
        }
        Presence::Value(value) => Some(value),
    };
    if let Some(identity) = &identity {
        validate_identity(identity)?;
    }
    if let Some(kind) = kind {
        match (kind, size_bytes) {
            (Kind::File, Some(None)) | (Kind::Directory, Some(Some(_))) => {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRow,
                    "row size_bytes does not match its kind",
                ));
            }
            _ => {}
        }
    }
    Ok(Row {
        identity,
        kind,
        size_bytes,
        modified_unix_ns,
    })
}

fn convert_output(raw: RawOutput) -> Result<Output, ProtocolError> {
    let scope = match raw.scope.as_str() {
        "membership" => OutputScope::Membership,
        "metadata" => OutputScope::Metadata,
        "ordered" => OutputScope::Ordered,
        "page" => OutputScope::Page,
        "viewport" => OutputScope::Viewport,
        _ => return Err(schema_error("output scope is not recognized")),
    };
    validate_digest(&raw.digest).map_err(|error| error.with_field("output.digest"))?;
    let continuation = match raw.continuation.as_str() {
        "more" => Continuation::More,
        "end" => Continuation::End,
        "not_applicable" => Continuation::NotApplicable,
        _ => return Err(schema_error("output continuation is not recognized")),
    };
    Ok(Output {
        scope,
        digest: raw.digest,
        row_count: raw.row_count,
        continuation,
    })
}

fn convert_status(raw: RawStatus) -> Result<Status, ProtocolError> {
    let kind = match raw.kind.as_str() {
        "success" => StatusKind::Success,
        "not_supported" => StatusKind::NotSupported,
        "error" => StatusKind::Error,
        "cancelled" => StatusKind::Cancelled,
        _ => return Err(schema_error("status kind is not recognized")),
    };
    let code = nullable_value(raw.code, "status.code")?;
    let message = nullable_value(raw.message, "status.message")?;
    match kind {
        StatusKind::Success if code.is_some() || message.is_some() => {
            return Err(schema_error("success status code and message must be null"));
        }
        StatusKind::Success => {}
        StatusKind::NotSupported | StatusKind::Error | StatusKind::Cancelled
            if code.as_deref().is_none_or(str::is_empty)
                || message.as_deref().is_none_or(str::is_empty) =>
        {
            return Err(schema_error("non-success status requires code and message"));
        }
        _ => {}
    }
    Ok(Status {
        kind,
        code,
        message,
    })
}

fn parse_requested_fields(values: &[String]) -> Result<Vec<Field>, ProtocolError> {
    if values.is_empty() {
        return Err(schema_error("requested_fields must not be empty"));
    }
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let field = Field::parse(value)?;
        if fields.contains(&field) {
            return Err(schema_error("requested_fields must not contain duplicates"));
        }
        fields.push(field);
    }
    if fields.first() != Some(&Field::Identity) || fields.get(1) != Some(&Field::Kind) {
        return Err(schema_error(
            "requested_fields must start with identity and kind",
        ));
    }
    if fields
        .windows(2)
        .any(|window| field_index(window[0]) >= field_index(window[1]))
    {
        return Err(schema_error("requested_fields must use canonical order"));
    }
    Ok(fields)
}

const fn field_index(field: Field) -> usize {
    match field {
        Field::Identity => 0,
        Field::Kind => 1,
        Field::SizeBytes => 2,
        Field::ModifiedUnixNs => 3,
    }
}

fn parse_sort(raw: RawSort) -> Result<Sort, ProtocolError> {
    match (raw.field.as_str(), raw.direction.as_str()) {
        ("provider_order", "none") => Ok(Sort::ProviderOrder),
        ("name", "ascending") => Ok(Sort::NameAscending),
        _ => Err(schema_error(
            "sort field and direction are not a version 1 pair",
        )),
    }
}

fn parse_filter(raw: RawFilter) -> Result<Filter, ProtocolError> {
    match raw.kind.as_str() {
        "none" if raw.value.is_none() && raw.case_sensitive.is_none() => Ok(Filter::None),
        "name_contains"
            if raw.value.as_deref() == Some("file-0001") && raw.case_sensitive == Some(true) =>
        {
            Ok(Filter::NameContains {
                value: "file-0001".to_string(),
                case_sensitive: true,
            })
        }
        _ => Err(schema_error("filter is not a supported version 1 value")),
    }
}

fn parse_cache(raw: RawCacheState) -> Result<CacheState, ProtocolError> {
    let process = match raw.process.as_str() {
        "cold" => ProcessCache::Cold,
        "warm" => ProcessCache::Warm,
        _ => return Err(schema_error("cache.process is not recognized")),
    };
    let filesystem = match raw.filesystem.as_str() {
        "controlled_cold" => FilesystemCache::ControlledCold,
        "fresh_copy" => FilesystemCache::FreshCopy,
        "warm" => FilesystemCache::Warm,
        "uncontrolled" => FilesystemCache::Uncontrolled,
        _ => return Err(schema_error("cache.filesystem is not recognized")),
    };
    let semantic = match raw.semantic.as_str() {
        "empty" => SemanticCache::Empty,
        "reset" => SemanticCache::Reset,
        "reused" => SemanticCache::Reused,
        _ => return Err(schema_error("cache.semantic is not recognized")),
    };
    Ok(CacheState {
        process,
        filesystem,
        semantic,
    })
}

fn parse_phase(value: &str) -> Result<Phase, ProtocolError> {
    match value {
        "sample.started" => Ok(Phase::SampleStarted),
        "action.started" => Ok(Phase::ActionStarted),
        "row.first" => Ok(Phase::RowFirst),
        "viewport.committed" => Ok(Phase::ViewportCommitted),
        "page.committed" => Ok(Phase::PageCommitted),
        "listing.completed" => Ok(Phase::ListingCompleted),
        "transform.completed" => Ok(Phase::TransformCompleted),
        "view.committed" => Ok(Phase::ViewCommitted),
        "action.completed" => Ok(Phase::ActionCompleted),
        "sample.completed" => Ok(Phase::SampleCompleted),
        _ => Err(ProtocolError::new(
            ErrorCode::InvalidPhase,
            "phase is not recognized",
        )),
    }
}

fn parse_unavailable(value: &str) -> Result<UnavailableReason, ProtocolError> {
    match value {
        "unsupported" => Ok(UnavailableReason::Unsupported),
        "permission_denied" => Ok(UnavailableReason::PermissionDenied),
        "not_observable" => Ok(UnavailableReason::NotObservable),
        "platform_unavailable" => Ok(UnavailableReason::PlatformUnavailable),
        _ => Err(schema_error("unavailable reason is not recognized")),
    }
}

fn required_nullable<T>(value: Presence<T>, field: &str) -> Result<Option<T>, ProtocolError> {
    match value {
        Presence::Missing => Err(schema_error(format!("{field} is required"))),
        Presence::Null => Ok(None),
        Presence::Value(value) => Ok(Some(value)),
    }
}

fn required_nullable_string(
    value: Presence<String>,
    field: &str,
) -> Result<Option<String>, ProtocolError> {
    let value = required_nullable(value, field)?;
    if let Some(value) = &value {
        validate_identifier(value).map_err(|error| error.with_field(field))?;
    }
    Ok(value)
}

fn nullable_value<T>(value: Presence<T>, field: &str) -> Result<Option<T>, ProtocolError> {
    required_nullable(value, field)
}

fn validate_identifier(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.len() > 128 {
        return Err(schema_error("identifier length is outside 1..=128"));
    }
    let mut bytes = value.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(schema_error(
            "identifier does not match the protocol syntax",
        ));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), ProtocolError> {
    if value.len() != DIGEST_PREFIX.len() + 64
        || !value.starts_with(DIGEST_PREFIX)
        || !value[DIGEST_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(schema_error("digest must match sha256:[0-9a-f]{64}"));
    }
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') || value.contains('\0') {
        return Err(ProtocolError::new(
            ErrorCode::InvalidRow,
            "identity is not a safe relative UTF-8 path",
        ));
    }
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRow,
                "identity contains an unsafe path segment",
            ));
        }
    }
    Ok(())
}

fn schema_error(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ErrorCode::InvalidSchema, message)
}

fn malformed(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ErrorCode::MalformedJson, message)
}
