//! # Versioned flat manifests
//!
//! This module validates the two normative flat fixtures and constructs their
//! expected rows without touching the filesystem. Manifest digests cover the
//! generation parameters and the fixed expected values, while row digests
//! cover the semantic observations adapters must report.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::canonical::{CanonicalRow, canonical_digest, digest_records};
use crate::{Field, FixtureReference, Kind};

const EXPECTED_METADATA: [&str; 2] = ["size_bytes", "modified_unix_ns"];
const EXPECTED_EXTENSIONS: [&str; 4] = ["rs", "txt", "log", "bin"];
const EXPECTED_GENERATOR_ID: &str = "flat-v1";
const EXPECTED_DIRECTORY_EVERY: u64 = 10;
const EXPECTED_HIDDEN_EVERY: u64 = 25;
const EXPECTED_SIZE_MULTIPLIER: u64 = 7919;
const EXPECTED_SIZE_MODULUS: u64 = 1_048_573;
const EXPECTED_SIZE_OFFSET: u64 = 1;
const EXPECTED_MODIFIED_BASE: i64 = 1_704_067_200_000_000_000;
const EXPECTED_MODIFIED_STEP: i64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestErrorCode {
    Io,
    MalformedJson,
    InvalidSchema,
    DigestMismatch,
    ExpectedMismatch,
}

impl ManifestErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "io_error",
            Self::MalformedJson => "malformed_json",
            Self::InvalidSchema => "invalid_schema",
            Self::DigestMismatch => "manifest_digest_mismatch",
            Self::ExpectedMismatch => "manifest_expectation_mismatch",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestError {
    code: ManifestErrorCode,
    message: String,
    path: Option<PathBuf>,
}

impl ManifestError {
    fn new(code: ManifestErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: None,
        }
    }

    fn with_path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }

    pub const fn code(&self) -> ManifestErrorCode {
        self.code
    }

    pub const fn code_str(&self) -> &'static str {
        self.code.as_str()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code_str())?;
        if let Some(path) = &self.path {
            write!(formatter, " ({})", path.display())?;
        }
        write!(formatter, ": {}", self.message)
    }
}

impl std::error::Error for ManifestError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratorParameters {
    pub id: String,
    pub directory_every: u64,
    pub hidden_every: u64,
    pub extensions: Vec<String>,
    pub size_multiplier: u64,
    pub size_modulus: u64,
    pub size_offset: u64,
    pub modified_base_unix_ns: i64,
    pub modified_step_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedDigests {
    pub membership_digest: String,
    pub metadata_digest: String,
    pub name_order_digest: String,
    pub name_viewport_digest: String,
    pub filter_count: Option<u64>,
    pub filter_order_digest: Option<String>,
    pub filter_viewport_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedManifest {
    id: String,
    generator: GeneratorParameters,
    entry_count: usize,
    requested_metadata: Vec<String>,
    expected: ExpectedDigests,
    manifest_digest: String,
    rows: Vec<CanonicalRow>,
    identity_rows: BTreeMap<String, CanonicalRow>,
}

impl ValidatedManifest {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ManifestError> {
        let raw: RawManifest =
            serde_json::from_slice(bytes).map_err(|error| match error.classify() {
                serde_json::error::Category::Data => ManifestError::new(
                    ManifestErrorCode::InvalidSchema,
                    "manifest does not match its schema",
                ),
                serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
                    ManifestError::new(
                        ManifestErrorCode::MalformedJson,
                        "manifest is not valid JSON",
                    )
                }
                serde_json::error::Category::Io => ManifestError::new(
                    ManifestErrorCode::MalformedJson,
                    "manifest could not be read",
                ),
            })?;
        validate_manifest(raw)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| {
            ManifestError::new(ManifestErrorCode::Io, error.to_string()).with_path(path)
        })?;
        Self::from_bytes(&bytes).map_err(|error| error.with_path(path))
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub fn generator(&self) -> &GeneratorParameters {
        &self.generator
    }

    pub fn requested_metadata(&self) -> &[String] {
        &self.requested_metadata
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn expected(&self) -> &ExpectedDigests {
        &self.expected
    }

    pub fn fixture_reference(&self) -> FixtureReference {
        FixtureReference {
            id: self.id.clone(),
            digest: self.manifest_digest.clone(),
        }
    }

    pub fn expected_rows(&self) -> Vec<CanonicalRow> {
        self.rows.clone()
    }

    pub fn expected_name_rows(&self) -> Vec<CanonicalRow> {
        let mut rows = self.rows.clone();
        rows.sort_by(|left, right| left.identity.as_bytes().cmp(right.identity.as_bytes()));
        rows
    }

    pub fn expected_filter_rows(&self) -> Option<Vec<CanonicalRow>> {
        self.expected.filter_count.map(|_| {
            self.expected_name_rows()
                .into_iter()
                .filter(|row| row.identity.contains("file-0001"))
                .collect()
        })
    }

    pub fn membership_digest(&self) -> &str {
        &self.expected.membership_digest
    }

    pub fn metadata_digest(&self) -> &str {
        &self.expected.metadata_digest
    }

    pub fn name_order_digest(&self) -> &str {
        &self.expected.name_order_digest
    }

    pub fn name_viewport_digest(&self) -> &str {
        &self.expected.name_viewport_digest
    }

    pub fn filter_order_digest(&self) -> Option<&str> {
        self.expected.filter_order_digest.as_deref()
    }

    pub fn filter_viewport_digest(&self) -> Option<&str> {
        self.expected.filter_viewport_digest.as_deref()
    }

    pub fn digest_rows(&self, scope: &str, fields: &[Field], rows: &[CanonicalRow]) -> String {
        canonical_digest(scope, fields, rows)
    }

    pub(crate) fn expected_row(&self, identity: &str) -> Option<&CanonicalRow> {
        self.identity_rows.get(identity)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: u64,
    id: String,
    generator: RawGenerator,
    entry_count: u64,
    requested_metadata: Vec<String>,
    expected: RawExpected,
    manifest_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGenerator {
    id: String,
    directory_every: u64,
    hidden_every: u64,
    extensions: Vec<String>,
    size_multiplier: u64,
    size_modulus: u64,
    size_offset: u64,
    modified_base_unix_ns: i64,
    modified_step_ns: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExpected {
    membership_digest: String,
    metadata_digest: String,
    name_order_digest: String,
    name_viewport_digest: String,
    filter_count: Option<u64>,
    filter_order_digest: Option<String>,
    filter_viewport_digest: Option<String>,
}

fn validate_manifest(raw: RawManifest) -> Result<ValidatedManifest, ManifestError> {
    if raw.schema_version != 1 {
        return Err(expected_error("schema_version must be 1"));
    }
    let entry_count = match raw.id.as_str() {
        "flat-10k-v1" if raw.entry_count == 10_000 => 10_000,
        "flat-100k-v1" if raw.entry_count == 100_000 => 100_000,
        "flat-10k-v1" | "flat-100k-v1" => {
            return Err(expected_error("manifest id and entry_count do not match"));
        }
        _ => {
            return Err(expected_error(
                "manifest id is not a normative flat fixture",
            ));
        }
    };
    validate_generator(&raw.generator)?;
    if raw.requested_metadata != EXPECTED_METADATA {
        return Err(expected_error(
            "requested_metadata does not match the flat fixture contract",
        ));
    }
    validate_digest(&raw.manifest_digest)?;
    for digest in [
        &raw.expected.membership_digest,
        &raw.expected.metadata_digest,
        &raw.expected.name_order_digest,
        &raw.expected.name_viewport_digest,
    ] {
        validate_digest(digest)?;
    }
    let filter_fields_present = raw.expected.filter_count.is_some()
        && raw.expected.filter_order_digest.is_some()
        && raw.expected.filter_viewport_digest.is_some();
    let filter_fields_absent = raw.expected.filter_count.is_none()
        && raw.expected.filter_order_digest.is_none()
        && raw.expected.filter_viewport_digest.is_none();
    if !filter_fields_present && !filter_fields_absent {
        return Err(expected_error(
            "filter expectations must be complete or absent",
        ));
    }
    if let Some(digest) = &raw.expected.filter_order_digest {
        validate_digest(digest)?;
    }
    if let Some(digest) = &raw.expected.filter_viewport_digest {
        validate_digest(digest)?;
    }
    let generator = GeneratorParameters {
        id: raw.generator.id,
        directory_every: raw.generator.directory_every,
        hidden_every: raw.generator.hidden_every,
        extensions: raw.generator.extensions,
        size_multiplier: raw.generator.size_multiplier,
        size_modulus: raw.generator.size_modulus,
        size_offset: raw.generator.size_offset,
        modified_base_unix_ns: raw.generator.modified_base_unix_ns,
        modified_step_ns: raw.generator.modified_step_ns,
    };
    let rows = generate_rows(entry_count, &generator)?;
    let expected = ExpectedDigests {
        membership_digest: raw.expected.membership_digest,
        metadata_digest: raw.expected.metadata_digest,
        name_order_digest: raw.expected.name_order_digest,
        name_viewport_digest: raw.expected.name_viewport_digest,
        filter_count: raw.expected.filter_count,
        filter_order_digest: raw.expected.filter_order_digest,
        filter_viewport_digest: raw.expected.filter_viewport_digest,
    };
    validate_expected_rows(&rows, &expected)?;
    let computed_manifest_digest = manifest_digest(&raw.id, entry_count, &generator, &expected);
    if computed_manifest_digest != raw.manifest_digest {
        return Err(ManifestError::new(
            ManifestErrorCode::DigestMismatch,
            "manifest_digest does not cover the ordered manifest records",
        ));
    }
    let identity_rows = rows
        .iter()
        .cloned()
        .map(|row| (row.identity.clone(), row))
        .collect();
    Ok(ValidatedManifest {
        id: raw.id,
        generator,
        entry_count,
        requested_metadata: EXPECTED_METADATA
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        expected,
        manifest_digest: raw.manifest_digest,
        rows,
        identity_rows,
    })
}

fn validate_generator(generator: &RawGenerator) -> Result<(), ManifestError> {
    if generator.id != EXPECTED_GENERATOR_ID
        || generator.directory_every != EXPECTED_DIRECTORY_EVERY
        || generator.hidden_every != EXPECTED_HIDDEN_EVERY
        || generator
            .extensions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != EXPECTED_EXTENSIONS
        || generator.size_multiplier != EXPECTED_SIZE_MULTIPLIER
        || generator.size_modulus != EXPECTED_SIZE_MODULUS
        || generator.size_offset != EXPECTED_SIZE_OFFSET
        || generator.modified_base_unix_ns != EXPECTED_MODIFIED_BASE
        || generator.modified_step_ns != EXPECTED_MODIFIED_STEP
    {
        return Err(expected_error(
            "generator parameters are not flat-v1 normative values",
        ));
    }
    Ok(())
}

fn generate_rows(
    entry_count: usize,
    generator: &GeneratorParameters,
) -> Result<Vec<CanonicalRow>, ManifestError> {
    let mut rows = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let index_u64 =
            u64::try_from(index).map_err(|_| expected_error("row index is outside u64"))?;
        let is_directory = index_u64 % generator.directory_every == 0;
        let hidden = index_u64 % generator.hidden_every == 0;
        let prefix = if hidden { "." } else { "" };
        let (name, kind, size_bytes) = if is_directory {
            (format!("{prefix}dir-{index:06}"), Kind::Directory, None)
        } else {
            let extension_index = usize::try_from(index_u64 % generator.extensions.len() as u64)
                .map_err(|_| expected_error("extension index is outside usize"))?;
            let extension = &generator.extensions[extension_index];
            let size = index_u64
                .checked_mul(generator.size_multiplier)
                .ok_or_else(|| expected_error("generated size overflowed"))?
                % generator.size_modulus
                + generator.size_offset;
            (
                format!("{prefix}file-{index:06}.{extension}"),
                Kind::File,
                Some(size),
            )
        };
        let modified = generator
            .modified_base_unix_ns
            .checked_add(
                generator
                    .modified_step_ns
                    .checked_mul(
                        i64::try_from(index_u64)
                            .map_err(|_| expected_error("timestamp index is outside i64"))?,
                    )
                    .ok_or_else(|| expected_error("generated timestamp overflowed"))?,
            )
            .ok_or_else(|| expected_error("generated timestamp overflowed"))?;
        rows.push(CanonicalRow::new(name, kind, size_bytes, modified));
    }
    Ok(rows)
}

fn validate_expected_rows(
    rows: &[CanonicalRow],
    expected: &ExpectedDigests,
) -> Result<(), ManifestError> {
    let membership = canonical_digest("membership", &[Field::Identity], rows);
    if membership != expected.membership_digest {
        return Err(expected_error(
            "membership_digest does not match generated rows",
        ));
    }
    let metadata_fields = [
        Field::Identity,
        Field::Kind,
        Field::SizeBytes,
        Field::ModifiedUnixNs,
    ];
    if canonical_digest("metadata", &metadata_fields, rows) != expected.metadata_digest {
        return Err(expected_error(
            "metadata_digest does not match generated rows",
        ));
    }
    let name_rows = sorted_rows(rows);
    if canonical_digest("ordered", &[Field::Identity, Field::Kind], &name_rows)
        != expected.name_order_digest
    {
        return Err(expected_error(
            "name_order_digest does not match generated rows",
        ));
    }
    if canonical_digest(
        "viewport",
        &[Field::Identity, Field::Kind],
        &name_rows[..40],
    ) != expected.name_viewport_digest
    {
        return Err(expected_error(
            "name_viewport_digest does not match generated rows",
        ));
    }
    if let Some(filter_count) = expected.filter_count {
        let filtered = name_rows
            .iter()
            .filter(|row| row.identity.contains("file-0001"))
            .cloned()
            .collect::<Vec<_>>();
        if filtered.len() as u64 != filter_count {
            return Err(expected_error("filter_count does not match generated rows"));
        }
        let filter_order = expected
            .filter_order_digest
            .as_deref()
            .ok_or_else(|| expected_error("filter_order_digest is missing"))?;
        if canonical_digest("ordered", &[Field::Identity, Field::Kind], &filtered) != filter_order {
            return Err(expected_error(
                "filter_order_digest does not match generated rows",
            ));
        }
        let filter_viewport = expected
            .filter_viewport_digest
            .as_deref()
            .ok_or_else(|| expected_error("filter_viewport_digest is missing"))?;
        if canonical_digest("viewport", &[Field::Identity, Field::Kind], &filtered[..40])
            != filter_viewport
        {
            return Err(expected_error(
                "filter_viewport_digest does not match generated rows",
            ));
        }
    }
    Ok(())
}

fn sorted_rows(rows: &[CanonicalRow]) -> Vec<CanonicalRow> {
    let mut sorted = rows.to_vec();
    sorted.sort_by(|left, right| left.identity.as_bytes().cmp(right.identity.as_bytes()));
    sorted
}

fn manifest_digest(
    id: &str,
    entry_count: usize,
    generator: &GeneratorParameters,
    expected: &ExpectedDigests,
) -> String {
    let records = vec![
        vec!["manifest_id".to_string(), id.to_string()],
        vec!["entry_count".to_string(), entry_count.to_string()],
        vec!["schema_version".to_string(), "1".to_string()],
        vec!["generator".to_string(), generator.id.clone()],
        vec![
            "directory_every".to_string(),
            generator.directory_every.to_string(),
        ],
        vec![
            "hidden_every".to_string(),
            generator.hidden_every.to_string(),
        ],
        vec!["extensions".to_string(), generator.extensions.join(",")],
        vec![
            "size_multiplier".to_string(),
            generator.size_multiplier.to_string(),
        ],
        vec![
            "size_modulus".to_string(),
            generator.size_modulus.to_string(),
        ],
        vec!["size_offset".to_string(), generator.size_offset.to_string()],
        vec![
            "modified_base_unix_ns".to_string(),
            generator.modified_base_unix_ns.to_string(),
        ],
        vec![
            "modified_step_ns".to_string(),
            generator.modified_step_ns.to_string(),
        ],
        vec![
            "membership_digest".to_string(),
            expected.membership_digest.clone(),
        ],
        vec![
            "metadata_digest".to_string(),
            expected.metadata_digest.clone(),
        ],
        vec![
            "name_order_digest".to_string(),
            expected.name_order_digest.clone(),
        ],
    ];
    digest_records("manifest", vec!["name", "value"], records)
}

fn validate_digest(value: &str) -> Result<(), ManifestError> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(expected_error("digest must match sha256:[0-9a-f]{64}"));
    }
    Ok(())
}

fn expected_error(message: impl Into<String>) -> ManifestError {
    ManifestError::new(ManifestErrorCode::ExpectedMismatch, message)
}
