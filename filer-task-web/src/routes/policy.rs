//! # Policy Route
//!
//! Returns the effective immutable project policy in the same shape clients
//! use for project configuration, without exposing the config file version.

use axum::{
    Json,
    extract::{Path, State},
};
use filer_task::project::TaskProject;

use crate::{
    app::AppState,
    dto::{PolicyMutationRequest, ProjectPolicyResponse},
    error::WebError,
    routes::blocking,
};

pub(crate) async fn get_policy(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
) -> Result<Json<ProjectPolicyResponse>, WebError> {
    let registered = state.registry.resolve(&project_name)?;
    Ok(Json(registered.task_project()?.policy().into()))
}

pub(crate) async fn mutate_policy(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
    Json(request): Json<PolicyMutationRequest>,
) -> Result<Json<ProjectPolicyResponse>, WebError> {
    let registered = state.registry.resolve(&project_name)?;
    let write_lock = registered.write_lock();
    let _guard = write_lock.lock().await;
    let registry = state.registry.clone();
    let response = blocking(move || {
        let project = registered.task_project()?.reload()?;
        let fresh = apply_mutation(&project, request)?;
        let response = fresh.policy().into();
        registry.replace_task_project(&project_name, fresh)?;
        Ok(response)
    })
    .await?;
    Ok(Json(response))
}

fn apply_mutation(
    project: &TaskProject,
    request: PolicyMutationRequest,
) -> Result<TaskProject, WebError> {
    let fresh = match request {
        PolicyMutationRequest::AddDomain { name, prefixes } => {
            project.add_domain(name, &prefixes)?
        }
        PolicyMutationRequest::RemoveDomain { name } => project.remove_domain(name)?,
        PolicyMutationRequest::AddPrefix { domain, prefix } => {
            project.add_prefix(domain, prefix)?
        }
        PolicyMutationRequest::RemovePrefix { domain, prefix } => {
            project.remove_prefix(domain, prefix)?
        }
        PolicyMutationRequest::AddTaskType {
            name,
            criteria,
            role,
        } => project.add_task_type(name, criteria.into(), role.map(Into::into))?,
        PolicyMutationRequest::RemoveTaskType { name } => project.remove_task_type(name)?,
        PolicyMutationRequest::AddTag { tag } => project.add_tag(tag)?,
        PolicyMutationRequest::RemoveTag { tag } => project.remove_tag(tag)?,
    };
    Ok(fresh)
}
