use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TaskType {
    Milestone,
    Epic,
    Feature,
    Bug,
    Refactor,
    TechDebt,
    TestDebt,
    Design,
    Docs,
}

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
    pub parent: Option<String>,
    pub milestone: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    pub risk: Option<Risk>,
    pub impact: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub whitepaper: Option<String>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    pub path: PathBuf,
    pub domain: String,
    #[serde(flatten)]
    pub metadata: TaskMetadata,
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
        match self {
            Self::Milestone => write!(f, "Milestone"),
            Self::Epic => write!(f, "Epic"),
            Self::Feature => write!(f, "Feature"),
            Self::Bug => write!(f, "Bug"),
            Self::Refactor => write!(f, "Refactor"),
            Self::TechDebt => write!(f, "TechDebt"),
            Self::TestDebt => write!(f, "TestDebt"),
            Self::Design => write!(f, "Design"),
            Self::Docs => write!(f, "Docs"),
        }
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
        match value {
            "Milestone" => Ok(Self::Milestone),
            "Epic" => Ok(Self::Epic),
            "Feature" => Ok(Self::Feature),
            "Bug" => Ok(Self::Bug),
            "Refactor" => Ok(Self::Refactor),
            "TechDebt" => Ok(Self::TechDebt),
            "TestDebt" => Ok(Self::TestDebt),
            "Design" => Ok(Self::Design),
            "Docs" => Ok(Self::Docs),
            _ => Err(format!("invalid task type: {value}")),
        }
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
