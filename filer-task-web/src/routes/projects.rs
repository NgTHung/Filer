//! # Project Registration Route
//!
//! Registers discovered or newly initialized task projects in the running
//! process. Persistence belongs to the storage layer, so this route changes
//! only the in-memory registry.

use axum::{Json, extract::State};
use filer_task::{
    project::{InitProjectOptions, TaskProject},
    repo::discover_project_root,
};

use crate::{
    app::AppState,
    dto::{ProjectSummary, RegisterProjectRequest},
    error::WebError,
    routes::blocking,
};

pub(crate) async fn register_project(
    State(state): State<AppState>,
    Json(request): Json<RegisterProjectRequest>,
) -> Result<Json<ProjectSummary>, WebError> {
    let registry = state.registry.clone();
    let summary = blocking(move || {
        let project = if request.init {
            TaskProject::init(request.path, InitProjectOptions::default())?
        } else {
            let root = discover_project_root(request.path)?;
            TaskProject::open(root)?
        };
        registry.register(project)?.summary()
    })
    .await?;
    Ok(Json(summary))
}
