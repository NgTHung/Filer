//! # Application Wiring
//!
//! Holds shared state and builds the API router. `router` returns only the JSON
//! API so tests can drive it without static assets; the binary adds a static
//! file fallback on top.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    routing::{get, post},
};
use tokio::sync::Mutex;

use crate::{error::WebError, registry::ProjectRegistry, routes};

#[derive(Clone)]
pub struct AppState {
    pub(crate) registry: Arc<ProjectRegistry>,
    pub(crate) write_lock: Arc<Mutex<()>>,
}

impl AppState {
    pub fn single(start: PathBuf) -> Result<Self, WebError> {
        Ok(Self {
            registry: Arc::new(ProjectRegistry::single(start)?),
            write_lock: Arc::new(Mutex::new(())),
        })
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/projects", get(routes::tasks::list_projects))
        .route("/api/tasks", get(routes::tasks::list_tasks))
        .route("/api/tasks/{id}", get(routes::tasks::get_task))
        .route("/api/tasks/{id}/start", post(routes::transitions::start))
        .route("/api/tasks/{id}/done", post(routes::transitions::done))
        .route("/api/tasks/{id}/block", post(routes::transitions::block))
        .route("/api/tasks/{id}/defer", post(routes::transitions::defer))
        .route("/api/tasks/{id}/obsolete", post(routes::transitions::obsolete))
        .with_state(state)
}
