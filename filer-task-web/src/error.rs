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
    PreconditionRequired(String),
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
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    issues: Vec<ValidationIssue>,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, error, project, issues) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message, None, Vec::new()),
            Self::PreconditionRequired(message) => {
                return (
                    StatusCode::PRECONDITION_REQUIRED,
                    Json(ErrorBody {
                        error: message,
                        code: Some("precondition_required".to_string()),
                        field: None,
                        context: None,
                        project: None,
                        issues: Vec::new(),
                    }),
                )
                    .into_response();
            }
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
            Self::Task(error) => {
                let (status, body) = task_error(error);
                return (status, Json(body)).into_response();
            }
        };
        (
            status,
            Json(ErrorBody {
                error,
                code: None,
                field: None,
                context: None,
                project,
                issues,
            }),
        )
            .into_response()
    }
}

fn task_error(error: TaskError) -> (StatusCode, ErrorBody) {
    let status = match &error {
        TaskError::TaskNotFound { .. } => StatusCode::NOT_FOUND,
        TaskError::CriterionContentMismatch { .. } => StatusCode::PRECONDITION_FAILED,
        TaskError::Json(_) => StatusCode::BAD_REQUEST,
        TaskError::Io { .. } | TaskError::ConfigIo { .. } | TaskError::ProjectNotFound { .. } => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    let code = error.code().to_string();
    let field = match code.as_str() {
        "id_exists" => Some("number".to_string()),
        "prefix_not_allowed" => Some("prefix".to_string()),
        "tag_rejected" => Some("tags".to_string()),
        _ => None,
    };
    let context = error.context();
    let message = error.to_string();
    let issues = match error {
        TaskError::Validation(errors) => errors.into_iter().map(ValidationIssue::from).collect(),
        _ => Vec::new(),
    };
    (
        status,
        ErrorBody {
            error: message,
            code: Some(code),
            field,
            context: Some(context),
            project: None,
            issues,
        },
    )
}
