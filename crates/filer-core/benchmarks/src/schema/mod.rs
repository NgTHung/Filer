//! # Protocol schema types
//!
//! The schema module keeps the public request and event projections small.
//! Framing, request conversion, event conversion, and lexical checks live in
//! private siblings so each change remains local.

use std::collections::BTreeMap;

pub(crate) mod encode;
mod event;
mod framing;
mod request;
mod validation;

pub use encode::{encode_event_line, encode_request_line};
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

impl ProcessCache {
    pub const ALL: [Self; 2] = [Self::Cold, Self::Warm];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemCache {
    ControlledCold,
    FreshCopy,
    Warm,
    Uncontrolled,
}

impl FilesystemCache {
    pub const ALL: [Self; 4] = [
        Self::ControlledCold,
        Self::FreshCopy,
        Self::Warm,
        Self::Uncontrolled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControlledCold => "controlled_cold",
            Self::FreshCopy => "fresh_copy",
            Self::Warm => "warm",
            Self::Uncontrolled => "uncontrolled",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCache {
    Empty,
    Reset,
    Reused,
}

impl SemanticCache {
    pub const ALL: [Self; 3] = [Self::Empty, Self::Reset, Self::Reused];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Reset => "reset",
            Self::Reused => "reused",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
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
        Self::ALL
            .into_iter()
            .find(|field| field.as_str() == value)
            .ok_or_else(|| validation::schema_error("requested_fields contains an unknown field"))
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
    pub const ALL: [Self; 10] = [
        Self::SampleStarted,
        Self::ActionStarted,
        Self::RowFirst,
        Self::ViewportCommitted,
        Self::PageCommitted,
        Self::ListingCompleted,
        Self::TransformCompleted,
        Self::ViewCommitted,
        Self::ActionCompleted,
        Self::SampleCompleted,
    ];

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

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
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

impl UnavailableReason {
    pub const ALL: [Self; 4] = [
        Self::Unsupported,
        Self::PermissionDenied,
        Self::NotObservable,
        Self::PlatformUnavailable,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::PermissionDenied => "permission_denied",
            Self::NotObservable => "not_observable",
            Self::PlatformUnavailable => "platform_unavailable",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
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

impl Kind {
    pub const ALL: [Self; 2] = [Self::File, Self::Directory];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
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

impl OutputScope {
    pub const ALL: [Self; 5] = [
        Self::Membership,
        Self::Metadata,
        Self::Ordered,
        Self::Page,
        Self::Viewport,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Membership => "membership",
            Self::Metadata => "metadata",
            Self::Ordered => "ordered",
            Self::Page => "page",
            Self::Viewport => "viewport",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Continuation {
    More,
    End,
    NotApplicable,
}

impl Continuation {
    pub const ALL: [Self; 3] = [Self::More, Self::End, Self::NotApplicable];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::More => "more",
            Self::End => "end",
            Self::NotApplicable => "not_applicable",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
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

impl StatusKind {
    pub const ALL: [Self; 4] = [
        Self::Success,
        Self::NotSupported,
        Self::Error,
        Self::Cancelled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NotSupported => "not_supported",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|variant| variant.as_str() == value)
    }
}
