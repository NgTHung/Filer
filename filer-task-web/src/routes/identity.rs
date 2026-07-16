//! # Identity Routes
//!
//! Creates or renames the cookie-backed identity used for write attribution.
//! A stale cookie starts a new identity because database resets and user
//! deletion intentionally return clients to onboarding.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, header::SET_COOKIE},
};
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    error::WebError,
    identity::{Actor, IDENTITY_COOKIE, resolve_optional, session_token},
    storage::{IdentitySession, StorageError, StoredIdentity},
};

const COOKIE_MAX_AGE_SECONDS: u64 = 365 * 24 * 60 * 60;

#[derive(Deserialize)]
pub(crate) struct IdentityRequest {
    username: String,
}

#[derive(Serialize)]
pub(crate) struct IdentityResponse {
    username: String,
}

pub(crate) async fn get_identity(actor: Actor) -> Json<IdentityResponse> {
    Json(IdentityResponse {
        username: actor.username,
    })
}

pub(crate) async fn put_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<IdentityRequest>,
) -> Result<(HeaderMap, Json<IdentityResponse>), WebError> {
    let username = validate_username(&request.username)?;
    let existing = resolve_optional(&headers, &state).await?;
    let (identity, token) = match existing {
        Some(actor) => rename_or_recover(&state, &headers, actor, &username).await?,
        None => create(&state, &username).await?,
    };
    let cookie = format!(
        "{IDENTITY_COOKIE}={token}; Max-Age={COOKIE_MAX_AGE_SECONDS}; HttpOnly; SameSite=Lax; Path=/"
    );
    let cookie = HeaderValue::from_str(&cookie)
        .map_err(|error| WebError::Internal(format!("identity cookie was invalid: {error}")))?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(SET_COOKIE, cookie);
    Ok((
        response_headers,
        Json(IdentityResponse {
            username: identity.username,
        }),
    ))
}

async fn rename_or_recover(
    state: &AppState,
    headers: &HeaderMap,
    actor: Actor,
    username: &str,
) -> Result<(StoredIdentity, String), WebError> {
    match state
        .storage()
        .rename_identity(actor.user_id, username)
        .await
    {
        Ok(identity) => {
            let token = session_token(headers)
                .ok_or_else(|| WebError::Internal("resolved identity had no cookie".to_string()))?;
            Ok((identity, token.to_string()))
        }
        Err(StorageError::IdentityNotFound(_)) => create(state, username).await,
        Err(error) => Err(error.into()),
    }
}

async fn create(state: &AppState, username: &str) -> Result<(StoredIdentity, String), WebError> {
    let IdentitySession {
        identity,
        session_token,
    } = state.storage().create_identity(username).await?;
    Ok((identity, session_token))
}

fn validate_username(value: &str) -> Result<String, WebError> {
    let username = value.trim();
    let length = username.chars().count();
    if !(1..=64).contains(&length) {
        return Err(WebError::InvalidUsername(
            "username must contain between 1 and 64 characters".to_string(),
        ));
    }
    if username.chars().any(char::is_control) {
        return Err(WebError::InvalidUsername(
            "username must not contain control characters".to_string(),
        ));
    }
    Ok(username.to_string())
}
