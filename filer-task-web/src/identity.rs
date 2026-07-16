//! # Request Identity
//!
//! Resolves the long-lived identity cookie into the stable actor attached to a
//! write. The extractor rejects absent and stale tokens before body extractors
//! can start mutation work, while identity onboarding can call the optional
//! resolver to recover from an old cookie.

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, header::COOKIE, request::Parts},
};

use crate::{app::AppState, error::WebError, storage::StoredIdentity};

pub const IDENTITY_COOKIE: &str = "filer_task_identity";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub user_id: i64,
    pub username: String,
}

impl From<StoredIdentity> for Actor {
    fn from(identity: StoredIdentity) -> Self {
        Self {
            user_id: identity.user_id,
            username: identity.username,
        }
    }
}

impl FromRequestParts<AppState> for Actor {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        resolve_optional(&parts.headers, state)
            .await?
            .ok_or(WebError::IdentityRequired)
    }
}

pub(crate) async fn resolve_optional(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<Option<Actor>, WebError> {
    let Some(token) = cookie_value(headers, IDENTITY_COOKIE) else {
        return Ok(None);
    };
    Ok(state
        .storage()
        .resolve_identity(token)
        .await?
        .map(Into::into))
}

pub(crate) fn session_token(headers: &HeaderMap) -> Option<&str> {
    cookie_value(headers, IDENTITY_COOKIE)
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .map(str::trim)
        .find_map(|cookie| {
            let (cookie_name, value) = cookie.split_once('=')?;
            (cookie_name == name).then_some(value)
        })
}
