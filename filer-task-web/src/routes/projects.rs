//! # Project Registration Route
//!
//! Registers discovered or newly initialized task projects and removes
//! registrations. App state serializes database and in-memory changes so both
//! views of the registry stay consistent.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use filer_task::{
    project::{InitProjectOptions, TaskProject},
    repo::discover_project_root,
};

use crate::{
    app::AppState,
    dto::{ProjectSummary, RegisterProjectRequest},
    error::WebError,
    identity::Actor,
    routes::blocking,
};

pub(crate) async fn register_project(
    State(state): State<AppState>,
    actor: Actor,
    Json(request): Json<RegisterProjectRequest>,
) -> Result<Json<ProjectSummary>, WebError> {
    let project = blocking(move || {
        Ok(if request.init {
            TaskProject::init(request.path, InitProjectOptions::default())?
        } else {
            let root = discover_project_root(request.path)?;
            TaskProject::open(root)?
        })
    })
    .await?;
    let registered = state.register_project(&actor, project).await?;
    let summary = blocking(move || registered.summary()).await?;
    Ok(Json(summary))
}

pub(crate) async fn deregister_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    actor: Actor,
) -> Result<StatusCode, WebError> {
    state.deregister_project(&actor, &project).await?;
    Ok(StatusCode::NO_CONTENT)
}
