//! # Transition Routes
//!
//! Each transition reuses the `filer-task` lifecycle function, which validates
//! the whole repo before writing. A single write lock serializes concurrent web
//! writes; it cannot guard against the CLI or git editing the same files, so the
//! next read re-validates and surfaces any inconsistency.

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, State},
};
use filer_task::{
    agent_context::{ShowView, build_show},
    error::TaskError,
    lifecycle::{block_task, defer_task, done_task, obsolete_task, start_task},
    project::TaskProject,
    validate::{require_valid_report, validate_repo},
};

use crate::{app::AppState, dto::ReasonRequest, error::WebError, routes::blocking};

pub async fn start(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ShowView>, WebError> {
    transition(state, id, start_task).await
}

pub async fn done(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ShowView>, WebError> {
    transition(state, id, done_task).await
}

pub async fn block(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, id, move |project, id| {
        block_task(project, id, &reason)
    })
    .await
}

pub async fn defer(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, id, move |project, id| {
        defer_task(project, id, &reason)
    })
    .await
}

pub async fn obsolete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, id, move |project, id| {
        obsolete_task(project, id, &reason)
    })
    .await
}

async fn transition<F>(state: AppState, id: String, op: F) -> Result<Json<ShowView>, WebError>
where
    F: FnOnce(&TaskProject, &str) -> Result<PathBuf, TaskError> + Send + 'static,
{
    let project = state.registry.resolve(None)?.clone();
    let _guard = state.write_lock.lock().await;
    let view = blocking(move || {
        op(&project, &id)?;
        let tasks = require_valid_report(validate_repo(&project)?)?;
        build_show(&project, &tasks, &id)
    })
    .await?;
    Ok(Json(view))
}
