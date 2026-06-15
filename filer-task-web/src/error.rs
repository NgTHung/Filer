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
use filer_task::error::TaskError;
use serde::Serialize;

#[derive(Debug)]
pub enum WebError {
    BadRequest(String),
    ProjectNotFound(String),
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    details: Vec<String>,
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, error, details) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message, Vec::new()),
            Self::ProjectNotFound(name) => (
                StatusCode::NOT_FOUND,
                format!("project {name} is not registered"),
                Vec::new(),
            ),
            Self::Task(TaskError::NotFound { id }) => {
                (StatusCode::NOT_FOUND, format!("task {id} does not exist"), Vec::new())
            }
            Self::Task(TaskError::Validation(errors)) => {
                let details = errors
                    .into_iter()
                    .map(|error| match error.path {
                        Some(path) => format!("{}: {}", path.display(), error.message),
                        None => error.message,
                    })
                    .collect();
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "task validation failed".to_string(),
                    details,
                )
            }
            // A rejected transition (for example marking Done with unchecked
            // criteria) is a precondition the caller can resolve, not a fault.
            Self::Task(TaskError::Message(message)) => {
                (StatusCode::UNPROCESSABLE_ENTITY, message, Vec::new())
            }
            Self::Task(TaskError::Json(error)) => {
                (StatusCode::BAD_REQUEST, error.to_string(), Vec::new())
            }
            Self::Task(error @ (TaskError::Io { .. } | TaskError::MissingRepoRoot { .. })) => {
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string(), Vec::new())
            }
        };
        (status, Json(ErrorBody { error, details })).into_response()
    }
}
