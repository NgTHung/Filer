//! # Application Wiring
//!
//! Holds shared state and builds the API router. `router` returns only the JSON
//! API so tests can drive it without static assets; the binary adds a static
//! file fallback on top.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use filer_task::project::TaskProject;
use tokio::sync::Mutex;

use crate::{
    error::WebError,
    identity::Actor,
    registry::{ProjectRegistry, RegisteredProject},
    routes,
    storage::Storage,
};

#[derive(Clone)]
pub struct AppState {
    pub(crate) registry: Arc<ProjectRegistry>,
    storage: Storage,
    registry_mutation: Arc<Mutex<()>>,
}

impl AppState {
    pub async fn load(storage: Storage) -> Result<Self, WebError> {
        let registrations = storage.project_registrations().await?;
        let registry =
            tokio::task::spawn_blocking(move || ProjectRegistry::from_registrations(registrations))
                .await
                .map_err(|join| WebError::Internal(format!("registry loader failed: {join}")))?;
        Ok(Self {
            registry: Arc::new(registry),
            storage,
            registry_mutation: Arc::new(Mutex::new(())),
        })
    }

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
            registry_mutation: Arc::new(Mutex::new(())),
        })
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub(crate) async fn register_project(
        &self,
        _actor: &Actor,
        task_project: TaskProject,
    ) -> Result<RegisteredProject, WebError> {
        let registered = RegisteredProject::new(task_project)?;
        let _mutation = self.registry_mutation.lock().await;
        self.registry.ensure_name_available(registered.name())?;
        self.storage
            .insert_project_registration(&registered.registration())
            .await?;
        self.registry.publish(registered.clone());
        Ok(registered)
    }

    pub(crate) async fn deregister_project(
        &self,
        _actor: &Actor,
        name: &str,
    ) -> Result<(), WebError> {
        let _mutation = self.registry_mutation.lock().await;
        self.registry.resolve(name)?;
        if !self.storage.delete_project_registration(name).await? {
            return Err(WebError::Internal(format!(
                "project {name} is registered in memory but missing from persistent storage"
            )));
        }
        self.registry.remove(name);
        Ok(())
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/identity",
            get(routes::identity::get_identity).put(routes::identity::put_identity),
        )
        .route(
            "/api/projects",
            get(routes::tasks::list_projects).post(routes::projects::register_project),
        )
        .route(
            "/api/projects/{project}",
            delete(routes::projects::deregister_project),
        )
        .route(
            "/api/projects/{project}/policy",
            get(routes::policy::get_policy).patch(routes::policy::mutate_policy),
        )
        .route(
            "/api/projects/{project}/tasks",
            get(routes::tasks::list_tasks).post(routes::task_writes::create_task),
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
            get(routes::tasks::get_task).patch(routes::task_writes::edit_task),
        )
        .route(
            "/api/projects/{project}/tasks/{id}/criteria/{index}",
            put(routes::task_writes::set_criterion),
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
