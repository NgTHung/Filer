//! # Filer benchmark protocol and fixtures
//!
//! This package validates benchmark request and event messages without joining
//! Filer's normal workspace. Later modules add deterministic manifests and
//! filesystem preparation behind the same public seams.
//!
//! ```
//! use filer_core_benchmarks::{parse_request_bytes, ErrorCode};
//!
//! let request = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/valid_request.json"));
//! let parsed = parse_request_bytes(request);
//! assert!(parsed.is_ok());
//! assert_eq!(ErrorCode::InvalidSchema.as_str(), "invalid_schema");
//! ```

mod canonical;
mod error;
mod fixtures;
mod manifests;
mod scenarios;
mod schema;
mod validator;

pub use canonical::{CanonicalRow, canonical_digest};
pub use error::{ErrorCode, ErrorContext, ProtocolError};
pub use fixtures::{
    FixtureError, FixtureErrorCode, PreparedFixture, prepare_fixture, prepare_fixture_from_path,
};
pub use manifests::{
    ExpectedDigests, GeneratorParameters, ManifestError, ManifestErrorCode, ValidatedManifest,
};
pub use schema::{
    Adapter, CacheState, Clock, Continuation, Counts, Environment, Event, Field, FilesystemCache,
    Filter, FixtureReference, Group, Implementation, Kind, MetricValue, Output, OutputScope, Phase,
    ProcessCache, Request, Row, SemanticCache, Sort, Status, StatusKind, UnavailableReason,
    parse_event_line, parse_event_lines, parse_request_bytes,
};
pub use validator::{
    DeclaredCapabilities, GateResult, RunValidator, SampleValidator, StructuralGates,
    ValidatedSample,
};
