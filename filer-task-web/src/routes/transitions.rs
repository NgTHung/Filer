//! # Transition Routes
//!
//! Each transition reuses the `filer-task` lifecycle function, which validates
//! the whole repo before writing. A single write lock serializes concurrent web
//! writes; it cannot guard against the CLI or git editing the same files, so the
//! next read re-validates and surfaces any inconsistency.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    Json,
    extract::{Path, State},
};
use filer_task::{
    agent_context::{ShowView, build_show},
    error::TaskError,
    lifecycle::{block_task, defer_task, done_task, obsolete_task, start_task},
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
    transition(state, id, move |root, id| block_task(root, id, &reason)).await
}

pub async fn defer(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, id, move |root, id| defer_task(root, id, &reason)).await
}

pub async fn obsolete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, id, move |root, id| obsolete_task(root, id, &reason)).await
}

async fn transition<F>(state: AppState, id: String, op: F) -> Result<Json<ShowView>, WebError>
where
    F: FnOnce(&FsPath, &str) -> Result<PathBuf, TaskError> + Send + 'static,
{
    let root = state.registry.resolve(None)?.to_path_buf();
    let _guard = state.write_lock.lock().await;
    let view = blocking(move || {
        op(&root, &id)?;
        let tasks = require_valid_report(validate_repo(&root)?)?;
        build_show(&root, &tasks, &id)
    })
    .await?;
    Ok(Json(view))
}
