use axum::{
    Router,
    http::{HeaderValue, header::COOKIE},
};
use std::sync::atomic::{AtomicU64, Ordering};
use tower_http::set_header::SetRequestHeaderLayer;

use filer_task_web::{
    app::{AppState, router},
    identity::IDENTITY_COOKIE,
    storage::Role,
};

/// Builds a router whose requests carry a fresh test identity that owns every
/// project in `state`. Tests about other behavior then pass the membership
/// guards; membership tests build their own users instead.
pub async fn authenticated_router(state: AppState) -> Router {
    static NEXT_USER: AtomicU64 = AtomicU64::new(1);
    let username = format!("Test User {}", NEXT_USER.fetch_add(1, Ordering::Relaxed));
    let session = state
        .storage()
        .create_identity(&username, "Test browser")
        .await
        .expect("test identity creates");
    own_every_project(&state, session.identity.user_id).await;
    let cookie = HeaderValue::from_str(&format!("{IDENTITY_COOKIE}={}", session.session_token))
        .expect("test identity cookie is valid");
    router(state).layer(SetRequestHeaderLayer::if_not_present(COOKIE, cookie))
}

// Registries built from roots exist only in memory, but memberships reference
// persisted registrations, so missing registrations are persisted first.
async fn own_every_project(state: &AppState, user_id: i64) {
    let storage = state.storage();
    let persisted: Vec<String> = storage
        .project_registrations()
        .await
        .expect("registrations load")
        .into_iter()
        .map(|registration| registration.name)
        .collect();
    for registration in state.project_registrations() {
        if !persisted.contains(&registration.name) {
            storage
                .insert_project_registration(&registration)
                .await
                .expect("test registration persists");
        }
        storage
            .grant_member(&registration.name, user_id, Role::Owner, None, false)
            .await
            .expect("test owner grants");
    }
}
