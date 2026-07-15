//! # Web Data Transfer Objects
//!
//! Keeps HTTP-specific request and response shapes separate from the task
//! library so its domain types do not acquire web serialization policy.

use std::path::PathBuf;

use filer_task::{
    error::ValidationError,
    model::{Priority, TaskType},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ReasonRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub domain: String,
    pub prefix: String,
    pub number: String,
    pub title: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub priority: Priority,
    pub milestone: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetCriterionRequest {
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationIssue {
    pub code: String,
    pub path: Option<PathBuf>,
    pub message: String,
    pub context: serde_json::Value,
}

impl From<ValidationError> for ValidationIssue {
    fn from(value: ValidationError) -> Self {
        Self {
            code: value.code,
            path: value.path,
            message: value.message,
            context: value.context,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub name: String,
    pub task_count: usize,
    pub domain_count: usize,
    pub broken: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ValidationIssue>,
}
