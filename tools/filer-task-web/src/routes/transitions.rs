//! # Transition Routes
//!
//! Each transition validates and resolves within its selected project. A
//! per-project lock keeps the mutation and refreshed response together without
//! serializing unrelated repositories.

use axum::{
    Json,
    extract::{Path, State},
};
use taskroot::{
    agent_context::ShowView,
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{block_task, defer_task, done_task, obsolete_task, start_task},
    project::TaskProject,
};

use crate::{
    access::ProjectEditor,
    app::AppState,
    dto::ReasonRequest,
    error::WebError,
    routes::{tasks::resolve_identity, write},
};

pub(crate) async fn start(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
) -> Result<Json<ShowView>, WebError> {
    transition(state, editor, id, "task.start", None, start_task).await
}

pub(crate) async fn done(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
) -> Result<Json<ShowView>, WebError> {
    transition(state, editor, id, "task.done", None, done_task).await
}

pub(crate) async fn block(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        editor,
        id,
        "task.block",
        detail,
        move |task_project, identity| block_task(task_project, identity, &reason),
    )
    .await
}

pub(crate) async fn defer(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        editor,
        id,
        "task.defer",
        detail,
        move |task_project, identity| defer_task(task_project, identity, &reason),
    )
    .await
}

pub(crate) async fn obsolete(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        editor,
        id,
        "task.obsolete",
        detail,
        move |task_project, identity| obsolete_task(task_project, identity, &reason),
    )
    .await
}

async fn transition<F>(
    state: AppState,
    editor: ProjectEditor,
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
    let view = write::mutate(state, editor, action, detail, move |project, tasks| {
        let identity = resolve_identity(project, tasks, &id)?;
        op(project, &identity)?;
        Ok(identity)
    })
    .await?;
    Ok(Json(view))
}
