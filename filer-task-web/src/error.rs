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

use crate::{dto::ValidationIssue, storage::StorageError};

#[derive(Debug)]
pub enum WebError {
    BadRequest(String),
    IdentityRequired,
    InvalidUsername(String),
    UsernameTaken,
    PairingUnknownUsername,
    PairingPinWrong,
    PairingPinExpired,
    PairingPinConsumed,
    PairingPinLocked,
    PreconditionRequired(String),
    DuplicateProjectName(String),
    InvalidNewProjectName(String),
    NameRequiresInit,
    InvalidProjectName(PathBuf),
    NoProjects,
    ProjectNotFound(String),
    ProjectBroken {
        name: String,
        issues: Vec<ValidationIssue>,
    },
    TaskEdit(TaskError),
    Task(TaskError),
    Storage(StorageError),
    Internal(String),
}

impl From<TaskError> for WebError {
    fn from(value: TaskError) -> Self {
        Self::Task(value)
    }
}

impl From<StorageError> for WebError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::UsernameTaken => Self::UsernameTaken,
            other => Self::Storage(other),
        }
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
            Self::IdentityRequired => {
                return client_error(
                    StatusCode::UNAUTHORIZED,
                    "pick a username before making changes",
                    "identity_required",
                    None,
                );
            }
            Self::InvalidUsername(message) => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    message,
                    "invalid_username",
                    Some("username"),
                );
            }
            Self::UsernameTaken => {
                return client_error(
                    StatusCode::CONFLICT,
                    "username is already in use",
                    "username_taken",
                    Some("username"),
                );
            }
            Self::PairingUnknownUsername => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "no identity uses that username",
                    "pairing_username_unknown",
                    Some("username"),
                );
            }
            Self::PairingPinWrong => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "that pairing code does not match the username",
                    "pairing_pin_wrong",
                    Some("pin"),
                );
            }
            Self::PairingPinExpired => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "that pairing code has expired, mint a fresh one",
                    "pairing_pin_expired",
                    Some("pin"),
                );
            }
            Self::PairingPinConsumed => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "that pairing code was already used",
                    "pairing_pin_consumed",
                    Some("pin"),
                );
            }
            Self::PairingPinLocked => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "too many failed pairing attempts, wait a few minutes before trying again",
                    "pairing_pin_locked",
                    Some("pin"),
                );
            }
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
            // The name travels as data, not only inside the message: a client
            // registering a path that resolves to an already-registered project
            // has no other way to learn which project to switch to, because the
            // root is discovered by walking up from the path it sent.
            Self::DuplicateProjectName(name) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorBody {
                        error: format!("project name {name:?} is registered more than once"),
                        code: Some("duplicate_project_name".to_string()),
                        field: None,
                        context: None,
                        project: Some(name),
                        issues: Vec::new(),
                    }),
                )
                    .into_response();
            }
            Self::InvalidNewProjectName(name) => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("{name:?} is not a directory name a project can be created under"),
                    "invalid_project_name",
                    Some("name"),
                );
            }
            Self::NameRequiresInit => {
                return client_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "a project can only be named while it is being created",
                    "name_requires_init",
                    Some("name"),
                );
            }
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
            Self::TaskEdit(error) => {
                let (status, body) = task_edit_error(error);
                return (status, Json(body)).into_response();
            }
            Self::Task(error) => {
                let (status, body) = task_error(error);
                return (status, Json(body)).into_response();
            }
            Self::Storage(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                None,
                Vec::new(),
            ),
            Self::Internal(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message, None, Vec::new())
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

fn client_error(
    status: StatusCode,
    error: impl Into<String>,
    code: &str,
    field: Option<&str>,
) -> Response {
    (
        status,
        Json(ErrorBody {
            error: error.into(),
            code: Some(code.to_string()),
            field: field.map(str::to_string),
            context: None,
            project: None,
            issues: Vec::new(),
        }),
    )
        .into_response()
}

fn task_edit_error(error: TaskError) -> (StatusCode, ErrorBody) {
    let TaskError::Validation(validation_errors) = error else {
        return task_error(error);
    };
    let promoted = validation_errors.first().map(|issue| {
        (
            issue.code.clone(),
            edit_issue_field(issue),
            issue.context.clone(),
        )
    });
    let message = TaskError::Validation(validation_errors.clone()).to_string();
    let issues = validation_errors
        .into_iter()
        .map(ValidationIssue::from)
        .collect();
    let (code, field, context) =
        promoted.unwrap_or_else(|| ("validation_failed".to_string(), None, serde_json::json!({})));
    (
        StatusCode::UNPROCESSABLE_ENTITY,
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

fn edit_issue_field(issue: &filer_task::error::ValidationError) -> Option<String> {
    if let Some(field) = issue.context.get("field").and_then(|value| value.as_str()) {
        return Some(field.to_string());
    }
    let message = issue.message.as_str();
    if message.starts_with("dependency ")
        || message.starts_with("duplicate dependency")
        || message.starts_with("dependency cycle detected:")
        || message == "task cannot depend on itself"
    {
        Some("depends_on".to_string())
    } else if message.starts_with("parent ") {
        Some("parent".to_string())
    } else if message.starts_with("milestone ") || message.starts_with("milestone-role task") {
        Some("milestone".to_string())
    } else {
        None
    }
}

fn task_error(error: TaskError) -> (StatusCode, ErrorBody) {
    let status = match &error {
        TaskError::TaskNotFound { .. } | TaskError::ProjectNotFound { .. } => StatusCode::NOT_FOUND,
        TaskError::CriterionContentMismatch { .. } => StatusCode::PRECONDITION_FAILED,
        TaskError::Json(_) => StatusCode::BAD_REQUEST,
        TaskError::Io { .. } | TaskError::ConfigIo { .. } => StatusCode::INTERNAL_SERVER_ERROR,
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
