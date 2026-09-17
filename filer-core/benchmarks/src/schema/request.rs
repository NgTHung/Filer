//! # Request conversion
//!
//! This private module owns strict request objects and their closed version 1
//! configuration values.

use serde::Deserialize;

use super::validation::{PROTOCOL_VERSION, schema_error, validate_digest, validate_identifier};
use super::{
    Adapter, CacheState, Clock, Field, FilesystemCache, Filter, FixtureReference, Group,
    Implementation, ProcessCache, Request, SemanticCache, Sort,
};
use crate::{ErrorCode, ProtocolError};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRequest {
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

pub(super) fn convert(raw: RawRequest) -> Result<Request, ProtocolError> {
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
        environment: super::Environment {
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
