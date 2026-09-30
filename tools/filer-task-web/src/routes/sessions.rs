//! # Session Routes
//!
//! Lists and revokes the acting user's browser sessions. The list marks the
//! session that made the request so a person can see which browser they are
//! in; revoking any other session cuts it off at its next request. Revoke of
//! the acting session is rejected here because that is a sign-out, not a
//! session-management action.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::{app::AppState, error::WebError, identity::Actor, storage::SessionSummary};

#[derive(Serialize)]
pub(crate) struct SessionView {
    id: i64,
    device_label: String,
    created_at: i64,
    last_seen: i64,
    current: bool,
}

#[derive(Serialize)]
pub(crate) struct SessionsResponse {
    sessions: Vec<SessionView>,
}

pub(crate) async fn list_sessions(
    State(state): State<AppState>,
    actor: Actor,
) -> Result<Json<SessionsResponse>, WebError> {
    let sessions = state.storage().list_sessions(actor.user_id).await?;
    let sessions = sessions
        .into_iter()
        .map(|session| to_view(session, actor.session_id))
        .collect();
    Ok(Json(SessionsResponse { sessions }))
}

pub(crate) async fn revoke_session(
    State(state): State<AppState>,
    actor: Actor,
    Path(session_id): Path<i64>,
) -> Result<StatusCode, WebError> {
    if session_id == actor.session_id {
        return Err(WebError::SessionRevokeCurrent);
    }
    if !state
        .storage()
        .revoke_session(actor.user_id, session_id)
        .await?
    {
        return Err(WebError::SessionNotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

fn to_view(session: SessionSummary, current_session_id: i64) -> SessionView {
    SessionView {
        id: session.session_id,
        device_label: session.device_label,
        created_at: session.created_at,
        last_seen: session.last_seen,
        current: session.session_id == current_session_id,
    }
}
