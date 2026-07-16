use axum::{
    Router,
    http::{HeaderValue, header::COOKIE},
};
use std::sync::atomic::{AtomicU64, Ordering};
use tower_http::set_header::SetRequestHeaderLayer;

use filer_task_web::{
    app::{AppState, router},
    identity::IDENTITY_COOKIE,
};

pub async fn authenticated_router(state: AppState) -> Router {
    static NEXT_USER: AtomicU64 = AtomicU64::new(1);
    let username = format!("Test User {}", NEXT_USER.fetch_add(1, Ordering::Relaxed));
    let session = state
        .storage()
        .create_identity(&username)
        .await
        .expect("test identity creates");
    let cookie = HeaderValue::from_str(&format!("{IDENTITY_COOKIE}={}", session.session_token))
        .expect("test identity cookie is valid");
    router(state).layer(SetRequestHeaderLayer::if_not_present(COOKIE, cookie))
}
