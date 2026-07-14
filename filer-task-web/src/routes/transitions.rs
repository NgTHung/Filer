//! # Transition Routes
//!
//! Each transition validates and resolves within its selected project. A
//! per-project lock keeps the mutation and refreshed response together without
//! serializing unrelated repositories.

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path, State},
};
use filer_task::{
    agent_context::{ShowView, build_show},
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{block_task, defer_task, done_task, obsolete_task, start_task},
    project::TaskProject,
};

use crate::{
    app::AppState,
    dto::ReasonRequest,
    error::WebError,
    routes::{blocking, tasks::resolve_identity},
};

pub(crate) async fn start(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Json<ShowView>, WebError> {
    transition(state, project, id, start_task).await
}

pub(crate) async fn done(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
) -> Result<Json<ShowView>, WebError> {
    transition(state, project, id, done_task).await
}

pub(crate) async fn block(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, project, id, move |task_project, identity| {
        block_task(task_project, identity, &reason)
    })
    .await
}

pub(crate) async fn defer(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, project, id, move |task_project, identity| {
        defer_task(task_project, identity, &reason)
    })
    .await
}

pub(crate) async fn obsolete(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    transition(state, project, id, move |task_project, identity| {
        obsolete_task(task_project, identity, &reason)
    })
    .await
}

async fn transition<F>(
    state: AppState,
    project_name: String,
    id: String,
    op: F,
) -> Result<Json<ShowView>, WebError>
where
    F: FnOnce(&TaskProject, &TaskIdentity) -> Result<PathBuf, TaskError> + Send + 'static,
{
    let registered = state.registry.resolve(&project_name)?.clone();
    let write_lock = registered.write_lock();
    let _guard = write_lock.lock().await;
    let view = blocking(move || {
        let before = registered.validate()?;
        let identity = resolve_identity(registered.task_project(), &before.tasks, &id)?;
        op(registered.task_project(), &identity)?;
        let after = registered.validate()?;
        Ok(build_show(
            registered.task_project(),
            &after.tasks,
            &identity,
            &after.warnings,
        )?)
    })
    .await?;
    Ok(Json(view))
}
