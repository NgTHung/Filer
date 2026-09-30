use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize, Serializer};

use crate::identity::TaskIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TaskStatus {
    #[serde(rename = "To Do")]
    ToDo,
    #[serde(rename = "In Progress")]
    InProgress,
    Blocked,
    Done,
    Deferred,
    Obsolete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

/// An owned task type name resolved through the active project policy.
///
/// The value stays open so configured projects can add types without changing
/// this crate. Policy-aware operations reject names that are not configured.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TaskType(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Risk {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TaskMetadata {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: Priority,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<Risk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitepaper: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub path: PathBuf,
    pub domain: String,
    pub metadata: TaskMetadata,
}

impl Task {
    pub fn identity(&self) -> TaskIdentity {
        TaskIdentity {
            domain: self.domain.clone(),
            id: self.metadata.id.clone(),
        }
    }

    pub fn qualified_id(&self) -> String {
        self.identity().to_string()
    }
}

impl Serialize for Task {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TaskOutput<'a> {
            path: &'a PathBuf,
            domain: &'a str,
            qualified_id: String,
            #[serde(flatten)]
            metadata: &'a TaskMetadata,
        }

        TaskOutput {
            path: &self.path,
            domain: &self.domain,
            qualified_id: self.qualified_id(),
            metadata: &self.metadata,
        }
        .serialize(serializer)
    }
}

impl TaskType {
    /// Create a task type value without applying a project policy.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the configured task type name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SortBy {
    Status,
    Priority,
    Id,
    Domain,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToDo => write!(f, "To Do"),
            Self::InProgress => write!(f, "In Progress"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Done => write!(f, "Done"),
            Self::Deferred => write!(f, "Deferred"),
            Self::Obsolete => write!(f, "Obsolete"),
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "High"),
            Self::Medium => write!(f, "Medium"),
            Self::Low => write!(f, "Low"),
        }
    }
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for Risk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "High"),
            Self::Medium => write!(f, "Medium"),
            Self::Low => write!(f, "Low"),
        }
    }
}

impl FromStr for TaskStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "To Do" => Ok(Self::ToDo),
            "In Progress" => Ok(Self::InProgress),
            "Blocked" => Ok(Self::Blocked),
            "Done" => Ok(Self::Done),
            "Deferred" => Ok(Self::Deferred),
            "Obsolete" => Ok(Self::Obsolete),
            _ => Err(format!("invalid task status: {value}")),
        }
    }
}

impl FromStr for Priority {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "High" => Ok(Self::High),
            "Medium" => Ok(Self::Medium),
            "Low" => Ok(Self::Low),
            _ => Err(format!("invalid priority: {value}")),
        }
    }
}

impl FromStr for TaskType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(value))
    }
}

impl From<&str> for TaskType {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TaskType {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl FromStr for Risk {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "High" => Ok(Self::High),
            "Medium" => Ok(Self::Medium),
            "Low" => Ok(Self::Low),
            _ => Err(format!("invalid risk: {value}")),
        }
    }
}
