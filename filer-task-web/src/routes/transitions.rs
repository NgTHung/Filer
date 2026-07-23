//! # Transition Routes
//!
//! Each transition validates and resolves within its selected project. A
//! per-project lock keeps the mutation and refreshed response together without
//! serializing unrelated repositories.

use axum::{
    Json,
    extract::{Path, State},
};
use filer_task::{
    agent_context::ShowView,
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{block_task, defer_task, done_task, obsolete_task, start_task},
    project::TaskProject,
};

use crate::{
    app::AppState,
    dto::ReasonRequest,
    error::WebError,
    identity::Actor,
    routes::{tasks::resolve_identity, write},
};

pub(crate) async fn start(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    actor: Actor,
) -> Result<Json<ShowView>, WebError> {
    transition(state, actor, project, id, "task.start", None, start_task).await
}

pub(crate) async fn done(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    actor: Actor,
) -> Result<Json<ShowView>, WebError> {
    transition(state, actor, project, id, "task.done", None, done_task).await
}

pub(crate) async fn block(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    actor: Actor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        actor,
        project,
        id,
        "task.block",
        detail,
        move |task_project, identity| block_task(task_project, identity, &reason),
    )
    .await
}

pub(crate) async fn defer(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    actor: Actor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        actor,
        project,
        id,
        "task.defer",
        detail,
        move |task_project, identity| defer_task(task_project, identity, &reason),
    )
    .await
}

pub(crate) async fn obsolete(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    actor: Actor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        actor,
        project,
        id,
        "task.obsolete",
        detail,
        move |task_project, identity| obsolete_task(task_project, identity, &reason),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn transition<F>(
    state: AppState,
    actor: Actor,
    project_name: String,
    id: String,
    action: &'static str,
    detail: Option<String>,
    op: F,
) -> Result<Json<ShowView>, WebError>
where
    F: FnOnce(&TaskProject, &TaskIdentity) -> Result<std::path::PathBuf, TaskError>
        + Send
        + 'static,
{
    let view = write::mutate(
        state,
        actor,
        project_name,
        action,
        detail,
        move |project, tasks| {
            let identity = resolve_identity(project, tasks, &id)?;
            op(project, &identity)?;
            Ok(identity)
        },
    )
    .await?;
    Ok(Json(view))
}
