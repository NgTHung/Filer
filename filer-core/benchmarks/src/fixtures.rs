//! # Filesystem fixture preparation
//!
//! This module creates one new fixture directory from a validated manifest and
//! verifies the resulting filesystem before returning ownership to the caller.
//! Readback uses the same canonical rows as protocol validation, so platform
//! metadata cannot silently change the benchmark's semantic identity.
//!
//! ```no_run
//! # use std::path::Path;
//! # use filer_core_benchmarks::{ValidatedManifest, prepare_fixture};
//! # fn example(manifest: &ValidatedManifest) -> Result<(), Box<dyn std::error::Error>> {
//! let fixture = prepare_fixture(manifest, Path::new("/tmp/filer-flat-10k"))?;
//! println!("prepared {}", fixture.root().display());
//! fixture.close()?;
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, FileTimes, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::Kind;
use crate::canonical::CanonicalRow;
use crate::manifests::{ManifestError, ValidatedManifest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureErrorCode {
    Manifest,
    TargetExists,
    InvalidIdentity,
    Io,
    MissingEntry,
    ExtraEntry,
    KindMismatch,
    SizeMismatch,
    TimestampMismatch,
    Cleanup,
}

impl FixtureErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::TargetExists => "target_exists",
            Self::InvalidIdentity => "invalid_identity",
            Self::Io => "io",
            Self::MissingEntry => "missing_entry",
            Self::ExtraEntry => "extra_entry",
            Self::KindMismatch => "kind_mismatch",
            Self::SizeMismatch => "size_mismatch",
            Self::TimestampMismatch => "timestamp_mismatch",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureError {
    code: FixtureErrorCode,
    path: Option<PathBuf>,
    message: String,
    cleanup_error: Option<String>,
}

impl FixtureError {
    fn new(code: FixtureErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            path: None,
            message: message.into(),
            cleanup_error: None,
        }
    }

    fn with_path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }

    fn with_cleanup(mut self, error: impl Into<String>) -> Self {
        self.cleanup_error = Some(error.into());
        self
    }

    pub const fn code(&self) -> FixtureErrorCode {
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

    pub fn cleanup_error(&self) -> Option<&str> {
        self.cleanup_error.as_deref()
    }
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code_str())?;
        if let Some(path) = &self.path {
            write!(formatter, " ({})", path.display())?;
        }
        write!(formatter, ": {}", self.message)?;
        if let Some(cleanup_error) = &self.cleanup_error {
            write!(formatter, "; cleanup failed: {cleanup_error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for FixtureError {}

#[derive(Debug)]
pub struct PreparedFixture {
    root: PathBuf,
    manifest_id: String,
    manifest_digest: String,
    rows: Vec<CanonicalRow>,
    closed: bool,
}

impl PreparedFixture {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn rows(&self) -> &[CanonicalRow] {
        &self.rows
    }

    pub fn verify(&self) -> Result<(), FixtureError> {
        verify_rows(&self.root, &self.rows)
    }

    pub fn close(mut self) -> Result<(), FixtureError> {
        self.closed = true;
        fs::remove_dir_all(&self.root).map_err(|error| {
            FixtureError::new(FixtureErrorCode::Cleanup, error.to_string()).with_path(&self.root)
        })
    }
}

impl Drop for PreparedFixture {
    fn drop(&mut self) {
        if !self.closed {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub fn prepare_fixture(
    manifest: &ValidatedManifest,
    target: impl AsRef<Path>,
) -> Result<PreparedFixture, FixtureError> {
    let target = target.as_ref();
    fs::create_dir(target).map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::AlreadyExists {
            FixtureErrorCode::TargetExists
        } else {
            FixtureErrorCode::Io
        };
        FixtureError::new(code, error.to_string()).with_path(target)
    })?;
    let rows = manifest.expected_rows();
    if let Err(error) = create_rows(target, &rows).and_then(|()| verify_rows(target, &rows)) {
        return Err(cleanup_failed_target(target, error));
    }
    Ok(PreparedFixture {
        root: target.to_path_buf(),
        manifest_id: manifest.id().to_string(),
        manifest_digest: manifest.manifest_digest().to_string(),
        rows,
        closed: false,
    })
}

pub fn prepare_fixture_from_path(
    manifest_path: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<PreparedFixture, FixtureError> {
    let manifest_path = manifest_path.as_ref();
    let manifest = ValidatedManifest::load(manifest_path).map_err(manifest_error)?;
    prepare_fixture(&manifest, target)
}

fn manifest_error(error: ManifestError) -> FixtureError {
    FixtureError::new(FixtureErrorCode::Manifest, error.to_string())
}

fn cleanup_failed_target(target: &Path, error: FixtureError) -> FixtureError {
    match fs::remove_dir_all(target) {
        Ok(()) => error,
        Err(cleanup) => error.with_cleanup(cleanup.to_string()),
    }
}

fn create_rows(root: &Path, rows: &[CanonicalRow]) -> Result<(), FixtureError> {
    for row in rows {
        let path = row_path(root, &row.identity)?;
        match row.kind {
            Kind::Directory => {
                fs::create_dir(&path)
                    .map_err(|error| io_error("create directory", &path, error))?;
                set_modified_time(&path, row.modified_unix_ns)?;
            }
            Kind::File => {
                let size = row.size_bytes.ok_or_else(|| {
                    FixtureError::new(
                        FixtureErrorCode::SizeMismatch,
                        "file manifest row has no size",
                    )
                    .with_path(&path)
                })?;
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|error| io_error("create file", &path, error))?;
                file.set_len(size)
                    .map_err(|error| io_error("set file length", &path, error))?;
                set_modified_time_with_file(&file, &path, row.modified_unix_ns)?;
            }
        }
    }
    Ok(())
}

fn verify_rows(root: &Path, rows: &[CanonicalRow]) -> Result<(), FixtureError> {
    let expected = rows
        .iter()
        .map(|row| row.identity.clone())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let entries = fs::read_dir(root).map_err(|error| io_error("read fixture root", root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error("read fixture entry", root, error))?;
        let name = entry.file_name().into_string().map_err(|_| {
            FixtureError::new(
                FixtureErrorCode::ExtraEntry,
                "fixture contains a non-UTF-8 entry name",
            )
            .with_path(&entry.path())
        })?;
        actual.insert(name);
    }
    if let Some(identity) = expected.difference(&actual).next() {
        return Err(
            FixtureError::new(FixtureErrorCode::MissingEntry, "fixture entry is missing")
                .with_path(&root.join(identity)),
        );
    }
    if let Some(identity) = actual.difference(&expected).next() {
        return Err(FixtureError::new(
            FixtureErrorCode::ExtraEntry,
            "fixture contains an extra entry",
        )
        .with_path(&root.join(identity)));
    }
    for row in rows {
        let path = row_path(root, &row.identity)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| io_error("read fixture metadata", &path, error))?;
        match row.kind {
            Kind::Directory if !metadata.file_type().is_dir() => {
                return Err(FixtureError::new(
                    FixtureErrorCode::KindMismatch,
                    "expected a directory",
                )
                .with_path(&path));
            }
            Kind::File if !metadata.file_type().is_file() => {
                return Err(FixtureError::new(
                    FixtureErrorCode::KindMismatch,
                    "expected a regular file",
                )
                .with_path(&path));
            }
            _ => {}
        }
        if let Some(expected_size) = row.size_bytes
            && metadata.len() != expected_size
        {
            return Err(FixtureError::new(
                FixtureErrorCode::SizeMismatch,
                format!("expected {expected_size} bytes, found {}", metadata.len()),
            )
            .with_path(&path));
        }
        let modified = metadata
            .modified()
            .map_err(|error| io_error("read modification time", &path, error))?;
        let actual_ns = unix_nanos(modified).map_err(|error| error.with_path(&path))?;
        if actual_ns != row.modified_unix_ns {
            return Err(FixtureError::new(
                FixtureErrorCode::TimestampMismatch,
                format!(
                    "expected {} nanoseconds, found {actual_ns}",
                    row.modified_unix_ns
                ),
            )
            .with_path(&path));
        }
    }
    Ok(())
}

fn row_path(root: &Path, identity: &str) -> Result<PathBuf, FixtureError> {
    let relative = Path::new(identity);
    if identity.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FixtureError::new(
            FixtureErrorCode::InvalidIdentity,
            "manifest identity is not a safe relative path",
        ));
    }
    Ok(root.join(relative))
}

fn set_modified_time(path: &Path, modified_unix_ns: i64) -> Result<(), FixtureError> {
    let file =
        File::open(path).map_err(|error| io_error("open directory for timestamps", path, error))?;
    set_modified_time_with_file(&file, path, modified_unix_ns)
}

fn set_modified_time_with_file(
    file: &File,
    path: &Path,
    modified_unix_ns: i64,
) -> Result<(), FixtureError> {
    let modified = system_time(modified_unix_ns).map_err(|error| error.with_path(path))?;
    file.set_times(FileTimes::new().set_modified(modified))
        .map_err(|error| io_error("set modification time", path, error))
}

fn system_time(nanoseconds: i64) -> Result<SystemTime, FixtureError> {
    let duration = Duration::from_nanos(nanoseconds.unsigned_abs());
    if nanoseconds.is_negative() {
        UNIX_EPOCH.checked_sub(duration).ok_or_else(|| {
            FixtureError::new(
                FixtureErrorCode::TimestampMismatch,
                "manifest timestamp is outside the host time range",
            )
        })
    } else {
        UNIX_EPOCH.checked_add(duration).ok_or_else(|| {
            FixtureError::new(
                FixtureErrorCode::TimestampMismatch,
                "manifest timestamp is outside the host time range",
            )
        })
    }
}

fn unix_nanos(time: SystemTime) -> Result<i64, FixtureError> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_nanos()).map_err(|_| {
            FixtureError::new(
                FixtureErrorCode::TimestampMismatch,
                "filesystem timestamp is outside the manifest range",
            )
        }),
        Err(error) => {
            let nanos = i64::try_from(error.duration().as_nanos()).map_err(|_| {
                FixtureError::new(
                    FixtureErrorCode::TimestampMismatch,
                    "filesystem timestamp is outside the manifest range",
                )
            })?;
            nanos.checked_neg().ok_or_else(|| {
                FixtureError::new(
                    FixtureErrorCode::TimestampMismatch,
                    "filesystem timestamp is outside the manifest range",
                )
            })
        }
    }
}

fn io_error(operation: &str, path: &Path, error: std::io::Error) -> FixtureError {
    FixtureError::new(FixtureErrorCode::Io, format!("{operation}: {error}")).with_path(path)
}
