//! # Web Errors
//!
//! Translates library and request failures into HTTP responses. A validation
//! failure is the client's problem to fix (the repo is inconsistent), so it
//! maps to 422 with the error list rather than a 500, which would imply a server
//! fault the user cannot act on.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::path::PathBuf;

use filer_task::error::TaskError;
use serde::Serialize;

use crate::dto::ValidationIssue;

#[derive(Debug)]
pub enum WebError {
    BadRequest(String),
    DuplicateProjectName(String),
    InvalidProjectName(PathBuf),
    NoProjects,
    ProjectNotFound(String),
    ProjectBroken {
        name: String,
        issues: Vec<ValidationIssue>,
    },
    Task(TaskError),
}

impl From<TaskError> for WebError {
    fn from(value: TaskError) -> Self {
        Self::Task(value)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    issues: Vec<ValidationIssue>,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, error, project, issues) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message, None, Vec::new()),
            Self::DuplicateProjectName(name) => (
                StatusCode::BAD_REQUEST,
                format!("project name {name:?} is registered more than once"),
                None,
                Vec::new(),
            ),
            Self::InvalidProjectName(root) => (
                StatusCode::BAD_REQUEST,
                format!(
                    "project root {} has no portable UTF-8 directory name",
                    root.display()
                ),
                None,
                Vec::new(),
            ),
            Self::NoProjects => (
                StatusCode::BAD_REQUEST,
                "at least one project root is required".to_string(),
                None,
                Vec::new(),
            ),
            Self::ProjectNotFound(name) => (
                StatusCode::NOT_FOUND,
                format!("project {name} is not registered"),
                None,
                Vec::new(),
            ),
            Self::ProjectBroken { name, issues } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("project {name} failed validation"),
                Some(name),
                issues,
            ),
            Self::Task(TaskError::TaskNotFound { reference, .. }) => (
                StatusCode::NOT_FOUND,
                format!("task {reference} does not exist"),
                None,
                Vec::new(),
            ),
            Self::Task(TaskError::Validation(errors)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "task validation failed".to_string(),
                None,
                errors.into_iter().map(ValidationIssue::from).collect(),
            ),
            // A rejected transition (for example marking Done with unchecked
            // criteria) is a precondition the caller can resolve, not a fault.
            Self::Task(TaskError::Message(message)) => {
                (StatusCode::UNPROCESSABLE_ENTITY, message, None, Vec::new())
            }
            Self::Task(TaskError::Json(error)) => {
                (StatusCode::BAD_REQUEST, error.to_string(), None, Vec::new())
            }
            Self::Task(
                error @ (TaskError::ConfigInvalidJson { .. }
                | TaskError::ConfigUnsupportedVersion { .. }
                | TaskError::ConfigDuplicate { .. }
                | TaskError::ConfigUnknownField { .. }
                | TaskError::ConfigInvalidValue { .. }),
            ) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                error.to_string(),
                None,
                Vec::new(),
            ),
            Self::Task(
                error @ (TaskError::Io { .. }
                | TaskError::ConfigIo { .. }
                | TaskError::ProjectNotFound { .. }),
            ) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                None,
                Vec::new(),
            ),
            Self::Task(error) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                error.to_string(),
                None,
                Vec::new(),
            ),
        };
        (
            status,
            Json(ErrorBody {
                error,
                project,
                issues,
            }),
        )
            .into_response()
    }
}
