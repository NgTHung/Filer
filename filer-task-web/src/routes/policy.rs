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
    identity::Actor,
    routes::blocking,
    storage::NewActivity,
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
    actor: Actor,
    Json(request): Json<PolicyMutationRequest>,
) -> Result<Json<ProjectPolicyResponse>, WebError> {
    let registered = state.registry.resolve(&project_name)?;
    let write_lock = registered.write_lock();
    let _guard = write_lock.lock().await;
    let registry = state.registry.clone();
    let (action, detail) = policy_entry(&request);
    let response = blocking(move || {
        let project = registered.task_project()?.reload()?;
        let fresh = apply_mutation(&project, request)?;
        let response = fresh.policy().into();
        registry.replace_task_project(&project_name, fresh)?;
        Ok((project_name, response))
    })
    .await?;
    let (project_name, response) = response;
    state
        .storage()
        .record_committed_activity(NewActivity {
            user_id: actor.user_id,
            username: &actor.username,
            project: &project_name,
            task_id: None,
            action,
            detail: Some(&detail),
        })
        .await;
    Ok(Json(response))
}

/// Names the audit action and the operand it applies to. The action already
/// states the verb, so the detail carries only what was changed, kept readable
/// because it renders verbatim in the Activity feed.
fn policy_entry(request: &PolicyMutationRequest) -> (&'static str, String) {
    match request {
        PolicyMutationRequest::AddDomain { name, prefixes } => (
            "policy.add_domain",
            format!("{name} [{}]", prefixes.join(", ")),
        ),
        PolicyMutationRequest::RemoveDomain { name } => ("policy.remove_domain", name.clone()),
        PolicyMutationRequest::AddPrefix { domain, prefix } => {
            ("policy.add_prefix", format!("{domain}/{prefix}"))
        }
        PolicyMutationRequest::RemovePrefix { domain, prefix } => {
            ("policy.remove_prefix", format!("{domain}/{prefix}"))
        }
        PolicyMutationRequest::AddTaskType { name, .. } => ("policy.add_task_type", name.clone()),
        PolicyMutationRequest::RemoveTaskType { name } => ("policy.remove_task_type", name.clone()),
        PolicyMutationRequest::AddTag { tag } => ("policy.add_tag", tag.clone()),
        PolicyMutationRequest::RemoveTag { tag } => ("policy.remove_tag", tag.clone()),
    }
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
