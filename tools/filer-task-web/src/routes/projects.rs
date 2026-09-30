//! # Project Registration Route
//!
//! Registers discovered or newly initialized task projects and removes
//! registrations. App state serializes database and in-memory changes so both
//! views of the registry stay consistent.

use std::{fs, path::PathBuf};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use taskroot::{
    error::TaskError,
    project::{InitProjectOptions, TaskProject},
    repo::discover_project_root,
};

use crate::{
    app::AppState,
    dto::{ProjectSummary, RegisterProjectRequest},
    error::WebError,
    identity::Actor,
    project_name::validated,
    routes::blocking,
};

pub(crate) async fn register_project(
    State(state): State<AppState>,
    actor: Actor,
    Json(request): Json<RegisterProjectRequest>,
) -> Result<Json<ProjectSummary>, WebError> {
    let named = match (&request.name, request.init) {
        (Some(name), true) => Some(request.path.join(validated(name)?)),
        (Some(_), false) => return Err(WebError::NameRequiresInit),
        (None, _) => None,
    };
    let project = blocking(move || {
        Ok(match named {
            Some(root) => create_or_open(root)?,
            None if request.init => TaskProject::init(request.path, InitProjectOptions::default())?,
            None => TaskProject::open(discover_project_root(request.path)?)?,
        })
    })
    .await?;
    let registered = state.register_project(&actor, project).await?;
    let summary = blocking(move || registered.summary()).await?;
    Ok(Json(summary))
}

// The named directory is the project root, so this is the one path the server
// creates for the caller. Finding a project already there means the caller
// named the one they wanted, so it opens instead of refusing. An unnamed init
// still requires the directory to exist, where a typo would otherwise leave an
// empty project behind.
fn create_or_open(root: PathBuf) -> Result<TaskProject, WebError> {
    fs::create_dir_all(&root).map_err(|error| {
        WebError::BadRequest(format!("could not create {}: {error}", root.display()))
    })?;
    match TaskProject::init(&root, InitProjectOptions::default()) {
        Err(TaskError::ProjectAlreadyExists { .. }) => Ok(TaskProject::open(root)?),
        result => Ok(result?),
    }
}

pub(crate) async fn deregister_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    actor: Actor,
) -> Result<StatusCode, WebError> {
    state.deregister_project(&actor, &project).await?;
    Ok(StatusCode::NO_CONTENT)
}
