//! # Web Data Transfer Objects
//!
//! Keeps HTTP-specific request and response shapes separate from the task
//! library so its domain types do not acquire web serialization policy.

use std::{collections::BTreeMap, path::PathBuf};

use filer_task::{
    error::ValidationError,
    lifecycle::{FieldPatch, TaskPatch},
    model::{Priority, Risk, TaskType},
    project::{
        CriteriaPolicy, DomainPolicy, ProjectPolicy, TagPolicy, TaskTypePolicy, TaskTypeRole,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterProjectRequest {
    pub path: PathBuf,
    #[serde(default)]
    pub init: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PolicyMutationRequest {
    AddDomain {
        name: String,
        prefixes: Vec<String>,
    },
    RemoveDomain {
        name: String,
    },
    AddPrefix {
        domain: String,
        prefix: String,
    },
    RemovePrefix {
        domain: String,
        prefix: String,
    },
    AddTaskType {
        name: String,
        criteria: CriteriaPolicyRequest,
        role: Option<TaskTypeRoleRequest>,
    },
    RemoveTaskType {
        name: String,
    },
    AddTag {
        tag: String,
    },
    RemoveTag {
        tag: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CriteriaPolicyRequest {
    Acceptance,
    Exit,
}

impl From<CriteriaPolicyRequest> for CriteriaPolicy {
    fn from(value: CriteriaPolicyRequest) -> Self {
        match value {
            CriteriaPolicyRequest::Acceptance => Self::Acceptance,
            CriteriaPolicyRequest::Exit => Self::Exit,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskTypeRoleRequest {
    Milestone,
}

impl From<TaskTypeRoleRequest> for TaskTypeRole {
    fn from(value: TaskTypeRoleRequest) -> Self {
        match value {
            TaskTypeRoleRequest::Milestone => Self::Milestone,
        }
    }
}

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

#[derive(Debug, Deserialize)]
pub struct EditTaskRequest {
    #[serde(default)]
    title: PatchField<String>,
    #[serde(default)]
    summary: PatchField<String>,
    #[serde(default)]
    sections: PatchField<BTreeMap<String, String>>,
    #[serde(default)]
    risk: NullablePatchField<Risk>,
    #[serde(default)]
    impact: NullablePatchField<String>,
    #[serde(default)]
    tags: PatchField<Vec<String>>,
    #[serde(default)]
    milestone: NullablePatchField<String>,
    #[serde(default)]
    parent: NullablePatchField<String>,
    #[serde(default)]
    depends_on: PatchField<Vec<String>>,
}

#[derive(Debug, Default)]
enum PatchField<T> {
    #[default]
    Keep,
    Set(T),
}

impl<'de, T> Deserialize<'de> for PatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Set)
    }
}

#[derive(Debug, Default)]
enum NullablePatchField<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

impl<'de, T> Deserialize<'de> for NullablePatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        })
    }
}

impl From<EditTaskRequest> for TaskPatch {
    fn from(request: EditTaskRequest) -> Self {
        Self {
            title: required_patch(request.title),
            summary: required_patch(request.summary),
            sections: required_patch(request.sections).unwrap_or_default(),
            risk: nullable_patch(request.risk),
            impact: nullable_patch(request.impact),
            tags: required_patch(request.tags),
            milestone: nullable_patch(request.milestone),
            parent: nullable_patch(request.parent),
            depends_on: required_patch(request.depends_on),
        }
    }
}

fn required_patch<T>(patch: PatchField<T>) -> Option<T> {
    match patch {
        PatchField::Keep => None,
        PatchField::Set(value) => Some(value),
    }
}

fn nullable_patch<T>(patch: NullablePatchField<T>) -> FieldPatch<T> {
    match patch {
        NullablePatchField::Keep => FieldPatch::Keep,
        NullablePatchField::Set(value) => FieldPatch::Set(value),
        NullablePatchField::Clear => FieldPatch::Clear,
    }
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
