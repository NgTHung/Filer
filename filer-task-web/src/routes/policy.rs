//! # Policy Route
//!
//! Returns the effective immutable project policy in the same shape clients
//! use for project configuration, without exposing the config file version.

use axum::{
    Json,
    extract::{Path, State},
};

use crate::{app::AppState, dto::ProjectPolicyResponse, error::WebError};

pub(crate) async fn get_policy(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
) -> Result<Json<ProjectPolicyResponse>, WebError> {
    let registered = state.registry.resolve(&project_name)?;
    Ok(Json(registered.task_project().policy().into()))
}
