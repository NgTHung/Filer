//! # Web Data Transfer Objects
//!
//! Keeps HTTP-specific request and response shapes separate from the task
//! library so its domain types do not acquire web serialization policy.

use std::{collections::BTreeMap, path::PathBuf};

use filer_task::{
    error::ValidationError,
    model::{Priority, TaskType},
    project::{
        CriteriaPolicy, DomainPolicy, ProjectPolicy, TagPolicy, TaskTypePolicy, TaskTypeRole,
    },
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

#[derive(Debug, Serialize)]
pub struct ProjectPolicyResponse {
    domains: BTreeMap<String, DomainPolicyResponse>,
    task_types: BTreeMap<String, TaskTypePolicyResponse>,
    tags: TagPolicyResponse,
}

impl From<&ProjectPolicy> for ProjectPolicyResponse {
    fn from(policy: &ProjectPolicy) -> Self {
        Self {
            domains: policy
                .domains()
                .iter()
                .map(|(name, policy)| (name.clone(), policy.into()))
                .collect(),
            task_types: policy
                .task_types()
                .iter()
                .map(|(name, policy)| (name.clone(), policy.into()))
                .collect(),
            tags: policy.tags().into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct DomainPolicyResponse {
    prefixes: Vec<String>,
}

impl From<&DomainPolicy> for DomainPolicyResponse {
    fn from(policy: &DomainPolicy) -> Self {
        Self {
            prefixes: policy.prefixes().to_vec(),
        }
    }
}

#[derive(Debug, Serialize)]
struct TaskTypePolicyResponse {
    criteria: CriteriaPolicyResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<TaskTypeRoleResponse>,
}

impl From<&TaskTypePolicy> for TaskTypePolicyResponse {
    fn from(policy: &TaskTypePolicy) -> Self {
        Self {
            criteria: policy.criteria().into(),
            role: policy.role().map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum CriteriaPolicyResponse {
    Acceptance,
    Exit,
}

impl From<CriteriaPolicy> for CriteriaPolicyResponse {
    fn from(policy: CriteriaPolicy) -> Self {
        match policy {
            CriteriaPolicy::Acceptance => Self::Acceptance,
            CriteriaPolicy::Exit => Self::Exit,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum TaskTypeRoleResponse {
    Milestone,
}

impl From<TaskTypeRole> for TaskTypeRoleResponse {
    fn from(role: TaskTypeRole) -> Self {
        match role {
            TaskTypeRole::Milestone => Self::Milestone,
        }
    }
}

#[derive(Debug, Serialize)]
struct TagPolicyResponse {
    policy: TagPolicyValueResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed: Option<Vec<String>>,
}

impl From<&TagPolicy> for TagPolicyResponse {
    fn from(policy: &TagPolicy) -> Self {
        match policy {
            TagPolicy::Open => Self {
                policy: TagPolicyValueResponse::Open,
                allowed: None,
            },
            TagPolicy::Strict { allowed } => Self {
                policy: TagPolicyValueResponse::Strict,
                allowed: Some(allowed.clone()),
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum TagPolicyValueResponse {
    Open,
    Strict,
}
