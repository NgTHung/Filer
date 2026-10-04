//! # Membership Routes
//!
//! Lists members, lets owners grant, change, and revoke roles and review access
//! requests, and lets any signed-in user ask for access to a project they
//! cannot change. Each change made here records an activity row, so the
//! project history shows who granted what.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    access::ProjectOwner,
    app::AppState,
    error::WebError,
    identity::Actor,
    storage::{AccessRequestRecord, MemberRecord, NewActivity, Role},
};

#[derive(Serialize)]
pub(crate) struct MemberView {
    user_id: i64,
    username: String,
    role: Role,
    granted_by: Option<String>,
    granted_at: i64,
}

#[derive(Serialize)]
pub(crate) struct MembersResponse {
    members: Vec<MemberView>,
}

#[derive(Serialize)]
pub(crate) struct AccessRequestView {
    user_id: i64,
    username: String,
    requested_at: i64,
}

#[derive(Serialize)]
pub(crate) struct AccessRequestsResponse {
    requests: Vec<AccessRequestView>,
}

#[derive(Deserialize)]
pub(crate) struct RoleRequest {
    role: Role,
}

pub(crate) async fn list_members(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<MembersResponse>, WebError> {
    state.registry.resolve(&project)?;
    members_response(&state, &project).await
}

pub(crate) async fn put_member(
    State(state): State<AppState>,
    Path((_project, user_id)): Path<(String, i64)>,
    owner: ProjectOwner,
    Json(body): Json<RoleRequest>,
) -> Result<Json<MembersResponse>, WebError> {
    let outcome = state
        .storage()
        .grant_member(
            &owner.project,
            user_id,
            body.role,
            Some(owner.actor.user_id),
            true,
        )
        .await?;
    let action = match outcome.previous {
        None => Some("member.grant"),
        Some(previous) if previous != body.role => Some("member.role"),
        Some(_) => None,
    };
    if let Some(action) = action {
        let detail = format!("{} as {}", outcome.username, body.role);
        record(&state, &owner, action, Some(&detail)).await;
    }
    members_response(&state, &owner.project).await
}

pub(crate) async fn delete_member(
    State(state): State<AppState>,
    Path((_project, user_id)): Path<(String, i64)>,
    owner: ProjectOwner,
) -> Result<StatusCode, WebError> {
    let removed = state
        .storage()
        .remove_member(&owner.project, user_id, true)
        .await?;
    if let Some(removed) = removed {
        record(&state, &owner, "member.revoke", Some(&removed.username)).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_access_requests(
    State(state): State<AppState>,
    owner: ProjectOwner,
) -> Result<Json<AccessRequestsResponse>, WebError> {
    let requests = state
        .storage()
        .list_access_requests(&owner.project)
        .await?
        .into_iter()
        .map(request_view)
        .collect();
    Ok(Json(AccessRequestsResponse { requests }))
}

pub(crate) async fn deny_access_request(
    State(state): State<AppState>,
    Path((_project, user_id)): Path<(String, i64)>,
    owner: ProjectOwner,
) -> Result<StatusCode, WebError> {
    let username = state
        .storage()
        .delete_access_request(&owner.project, user_id)
        .await?
        .ok_or(WebError::AccessRequestNotFound)?;
    record(&state, &owner, "access.deny", Some(&username)).await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn request_access(
    State(state): State<AppState>,
    Path(project): Path<String>,
    actor: Actor,
) -> Result<StatusCode, WebError> {
    state.registry.resolve(&project)?;
    if state
        .storage()
        .request_access(&project, actor.user_id)
        .await?
    {
        state
            .storage()
            .record_committed_activity(NewActivity {
                user_id: actor.user_id,
                username: &actor.username,
                project: &project,
                task_id: None,
                action: "access.request",
                detail: None,
            })
            .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn withdraw_access_request(
    State(state): State<AppState>,
    Path(project): Path<String>,
    actor: Actor,
) -> Result<StatusCode, WebError> {
    state.registry.resolve(&project)?;
    state
        .storage()
        .delete_access_request(&project, actor.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn members_response(
    state: &AppState,
    project: &str,
) -> Result<Json<MembersResponse>, WebError> {
    let members = state
        .storage()
        .list_members(project)
        .await?
        .into_iter()
        .map(member_view)
        .collect();
    Ok(Json(MembersResponse { members }))
}

async fn record(state: &AppState, owner: &ProjectOwner, action: &str, detail: Option<&str>) {
    state
        .storage()
        .record_committed_activity(NewActivity {
            user_id: owner.actor.user_id,
            username: &owner.actor.username,
            project: &owner.project,
            task_id: None,
            action,
            detail,
        })
        .await;
}

fn member_view(member: MemberRecord) -> MemberView {
    MemberView {
        user_id: member.user_id,
        username: member.username,
        role: member.role,
        granted_by: member.granted_by,
        granted_at: member.granted_at,
    }
}

fn request_view(request: AccessRequestRecord) -> AccessRequestView {
    AccessRequestView {
        user_id: request.user_id,
        username: request.username,
        requested_at: request.requested_at,
    }
}
