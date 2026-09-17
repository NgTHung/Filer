//! # Protocol schema types
//!
//! The schema module keeps the public request and event projections small.
//! Framing, request conversion, event conversion, and lexical checks live in
//! private siblings so each change remains local.

use std::collections::BTreeMap;

mod event;
mod framing;
mod request;
mod validation;

pub use framing::{parse_event_line, parse_event_lines, parse_request_bytes};
pub(crate) use validation::{is_valid_digest, is_valid_identifier};

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

    fn parse(value: &str) -> Result<Self, crate::ProtocolError> {
        match value {
            "identity" => Ok(Self::Identity),
            "kind" => Ok(Self::Kind),
            "size_bytes" => Ok(Self::SizeBytes),
            "modified_unix_ns" => Ok(Self::ModifiedUnixNs),
            _ => Err(validation::schema_error(
                "requested_fields contains an unknown field",
            )),
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
