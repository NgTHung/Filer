//! # Access Guards
//!
//! Extractors that turn a session into a permission check. A write handler
//! names the guard it needs in its signature. Each guard runs before body
//! extractors, so a forbidden request is rejected before its JSON is parsed or
//! the project's write lock is taken. Checks run in order: no session is 401,
//! an unregistered project is 404, and a missing role is 403.
//!
//! ```ignore
//! async fn start(editor: ProjectEditor, Json(body): Json<Request>) { /* ... */ }
//! ```

use std::net::IpAddr;

use axum::{
    extract::{FromRequestParts, RawPathParams},
    http::request::Parts,
};

use crate::{app::AppState, error::WebError, identity::Actor, storage::Role};

/// A signed-in server admin. Admin controls project registration only.
pub struct ServerAdmin(pub Actor);

/// A signed-in user holding the editor or owner role on the `{project}` in the
/// request path.
pub struct ProjectEditor {
    pub actor: Actor,
    pub project: String,
}

/// A signed-in user holding the owner role on the `{project}` in the request
/// path.
pub struct ProjectOwner {
    pub actor: Actor,
    pub project: String,
}

impl FromRequestParts<AppState> for ServerAdmin {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let actor = Actor::from_request_parts(parts, state).await?;
        if !actor.is_admin {
            return Err(WebError::AdminRequired);
        }
        Ok(Self(actor))
    }
}

impl FromRequestParts<AppState> for ProjectEditor {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let (actor, project) = require_project_role(parts, state, Role::Editor).await?;
        Ok(Self { actor, project })
    }
}

impl FromRequestParts<AppState> for ProjectOwner {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let (actor, project) = require_project_role(parts, state, Role::Owner).await?;
        Ok(Self { actor, project })
    }
}

async fn require_project_role(
    parts: &mut Parts,
    state: &AppState,
    required: Role,
) -> Result<(Actor, String), WebError> {
    let actor = Actor::from_request_parts(parts, state).await?;
    let project = project_param(parts, state).await?;
    state.registry.resolve(&project)?;
    let role = state
        .storage()
        .project_role(&project, actor.user_id)
        .await?;
    if !role.is_some_and(|role| role.satisfies(required)) {
        return Err(WebError::ProjectRoleRequired(required));
    }
    Ok((actor, project))
}

// RawPathParams yields percent-decoded values, so the name matches the
// registry even when the URI encodes characters such as spaces.
async fn project_param(parts: &mut Parts, state: &AppState) -> Result<String, WebError> {
    let params = RawPathParams::from_request_parts(parts, state)
        .await
        .map_err(|rejection| WebError::BadRequest(rejection.body_text()))?;
    params
        .iter()
        .find(|(key, _)| *key == "project")
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| {
            WebError::Internal(
                "a project guard is mounted on a route without {project}".to_string(),
            )
        })
}

/// The startup warning for a listen address other machines can reach, or
/// `None` for a loopback address.
pub fn exposure_warning(host: IpAddr) -> Option<String> {
    (!host.is_loopback()).then(|| {
        format!(
            "warning: listening on {host}: reads are open to anyone who can reach this address; writes need project membership"
        )
    })
}
