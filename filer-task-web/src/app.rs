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

use crate::{error::WebError, registry::ProjectRegistry, routes, storage::Storage};

#[derive(Clone)]
pub struct AppState {
    pub(crate) registry: Arc<ProjectRegistry>,
    storage: Storage,
}

impl AppState {
    pub fn single(start: PathBuf, storage: Storage) -> Result<Self, WebError> {
        Self::from_roots([start], storage)
    }

    pub fn from_roots(
        roots: impl IntoIterator<Item = PathBuf>,
        storage: Storage,
    ) -> Result<Self, WebError> {
        Ok(Self {
            registry: Arc::new(ProjectRegistry::from_roots(roots)?),
            storage,
        })
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/projects", get(routes::tasks::list_projects))
        .route(
            "/api/projects/{project}/tasks",
            get(routes::tasks::list_tasks),
        )
        .route(
            "/api/projects/{project}/ready",
            get(routes::tasks::list_ready),
        )
        .route(
            "/api/projects/{project}/milestones",
            get(routes::tasks::list_milestones),
        )
        .route(
            "/api/projects/{project}/tasks/{id}",
            get(routes::tasks::get_task),
        )
        .route(
            "/api/projects/{project}/tasks/{id}/start",
            post(routes::transitions::start),
        )
        .route(
            "/api/projects/{project}/tasks/{id}/done",
            post(routes::transitions::done),
        )
        .route(
            "/api/projects/{project}/tasks/{id}/block",
            post(routes::transitions::block),
        )
        .route(
            "/api/projects/{project}/tasks/{id}/defer",
            post(routes::transitions::defer),
        )
        .route(
            "/api/projects/{project}/tasks/{id}/obsolete",
            post(routes::transitions::obsolete),
        )
        .with_state(state)
}
