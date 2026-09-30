//! # Context Route
//!
//! Serves the graph view the detail drawer navigates by: relationships,
//! ancestors, and readiness blockers. It is a separate endpoint from `show`
//! because building the relations costs a full task graph, and list-driven
//! reads should not pay for it.

use axum::{
    Json,
    extract::{Path, State},
};
use taskroot::agent_context::{ContextView, build_context};

use crate::{app::AppState, error::WebError, routes::blocking, routes::tasks::resolve_identity};

pub(crate) async fn get_context(
    State(state): State<AppState>,
    Path((project_name, id)): Path<(String, String)>,
) -> Result<Json<ContextView>, WebError> {
    let registered = state.registry.resolve(&project_name)?.clone();
    let view = blocking(move || {
        let validated = registered.validate()?;
        let identity = resolve_identity(registered.task_project()?, &validated.tasks, &id)?;
        Ok(build_context(
            registered.task_project()?,
            &validated.tasks,
            &identity,
            &validated.warnings,
        )?)
    })
    .await?;
    Ok(Json(view))
}
