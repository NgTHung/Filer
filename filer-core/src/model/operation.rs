use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Copy,
    Move,
    Delete,
    Rename,
    CreateFolder,
    CreateFile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationConflictResolution {
    Fail,
    Replace,
    Skip,
    RenameIncoming,
    RenameExisting,
    MergeDirectory,
    Unknown(String),
}

impl OperationConflictResolution {
    fn as_wire_value(&self) -> &str {
        match self {
            Self::Fail => "fail",
            Self::Replace => "replace",
            Self::Skip => "skip",
            Self::RenameIncoming => "rename_incoming",
            Self::RenameExisting => "rename_existing",
            Self::MergeDirectory => "merge_directory",
            Self::Unknown(value) => value.as_str(),
        }
    }

    fn from_wire_value(value: &str) -> Self {
        match value {
            "fail" => Self::Fail,
            "replace" => Self::Replace,
            "skip" => Self::Skip,
            "rename_incoming" => Self::RenameIncoming,
            "rename_existing" => Self::RenameExisting,
            "merge_directory" => Self::MergeDirectory,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl Serialize for OperationConflictResolution {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_wire_value())
    }
}

impl<'de> Deserialize<'de> for OperationConflictResolution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StringEnumVisitor::new(
            "operation conflict resolution",
            |value| OperationConflictResolution::from_wire_value(value),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationConflictPolicy {
    pub default: OperationConflictResolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<OperationConflictResolution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<OperationConflictResolution>,
}

impl Default for OperationConflictPolicy {
    fn default() -> Self {
        Self {
            default: OperationConflictResolution::Fail,
            file: None,
            directory: None,
        }
    }
}

impl OperationConflictPolicy {
    pub fn file_resolution(&self) -> OperationConflictResolution {
        self.file.clone().unwrap_or_else(|| self.default.clone())
    }

    pub fn directory_resolution(&self) -> OperationConflictResolution {
        self.directory
            .clone()
            .unwrap_or_else(|| self.default.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationProviderGuarantee {
    Atomic,
    BestEffort,
    Unsupported,
    Unknown(String),
}

impl OperationProviderGuarantee {
    fn as_wire_value(&self) -> &str {
        match self {
            Self::Atomic => "atomic",
            Self::BestEffort => "best_effort",
            Self::Unsupported => "unsupported",
            Self::Unknown(value) => value.as_str(),
        }
    }

    fn from_wire_value(value: &str) -> Self {
        match value {
            "atomic" => Self::Atomic,
            "best_effort" => Self::BestEffort,
            "unsupported" => Self::Unsupported,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl Serialize for OperationProviderGuarantee {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_wire_value())
    }
}

impl<'de> Deserialize<'de> for OperationProviderGuarantee {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StringEnumVisitor::new(
            "operation provider guarantee",
            |value| OperationProviderGuarantee::from_wire_value(value),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationUndoMode {
    Unavailable,
    BestEffort,
    ProviderAtomic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationUndoRecord {
    pub operation: OperationKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<crate::model::location::LocationRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub destinations: Vec<crate::model::location::LocationRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected: Vec<crate::model::location::LocationRef>,
    pub trash: bool,
    pub undo: OperationUndoMode,
    pub guarantee: OperationProviderGuarantee,
}

/// Unique identifier for a file operation flow.
///
/// Operation IDs are runtime-local correlation tokens. They are monotonic to
/// keep logs, tests, progress updates, and completion events easy to reason
/// about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub u64);

impl OperationId {
    /// Generate a new unique operation ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Default operation ID for compatibility placeholders.
    pub const DEFAULT: OperationId = OperationId(0);
}

impl Default for OperationId {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation:{}", self.0)
    }
}

struct StringEnumVisitor<T, F>
where
    F: Fn(&str) -> T,
{
    name: &'static str,
    parse: F,
}

impl<T, F> StringEnumVisitor<T, F>
where
    F: Fn(&str) -> T,
{
    fn new(name: &'static str, parse: F) -> Self {
        Self { name, parse }
    }
}

impl<'de, T, F> Visitor<'de> for StringEnumVisitor<T, F>
where
    F: Fn(&str) -> T,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "a {}", self.name)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok((self.parse)(value))
    }
}
