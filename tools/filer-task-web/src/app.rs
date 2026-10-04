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
use taskroot::project::TaskProject;
use tokio::sync::Mutex;

use crate::{
    error::WebError,
    identity::Actor,
    registry::{ProjectRegistry, RegisteredProject},
    routes,
    storage::{NewActivity, ProjectRegistration, Storage},
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

    /// The registration of every project the registry holds, in registry order.
    pub fn project_registrations(&self) -> Vec<ProjectRegistration> {
        self.registry
            .names()
            .into_iter()
            .filter_map(|name| self.registry.resolve(&name).ok())
            .map(|project| project.registration())
            .collect()
    }

    pub(crate) async fn register_project(
        &self,
        actor: &Actor,
        task_project: TaskProject,
    ) -> Result<RegisteredProject, WebError> {
        let registered = RegisteredProject::new(task_project)?;
        let _mutation = self.registry_mutation.lock().await;
        self.registry.ensure_name_available(registered.name())?;
        let registration = registered.registration();
        self.storage
            .insert_project_registration_with_owner(&registration, actor.user_id)
            .await?;
        self.registry.publish(registered.clone());
        self.storage
            .record_committed_activity(NewActivity {
                user_id: actor.user_id,
                username: &actor.username,
                project: &registration.name,
                task_id: None,
                action: "project.register",
                detail: Some(&registration.root.display().to_string()),
            })
            .await;
        Ok(registered)
    }

    pub(crate) async fn deregister_project(
        &self,
        actor: &Actor,
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
        self.storage
            .record_committed_activity(NewActivity {
                user_id: actor.user_id,
                username: &actor.username,
                project: name,
                task_id: None,
                action: "project.deregister",
                detail: None,
            })
            .await;
        Ok(())
    }
}

// Every non-GET route below names its guard in the handler signature, and
// `write_routes` in tests/membership_api_test.rs lists each one with that
// guard. A non-GET route added here joins that table.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/identity",
            get(routes::identity::get_identity).put(routes::identity::put_identity),
        )
        .route(
            "/api/identity/pin",
            post(routes::identity::mint_pairing_pin),
        )
        .route("/api/identity/pair", post(routes::identity::pair_identity))
        .route("/api/sessions", get(routes::sessions::list_sessions))
        .route(
            "/api/sessions/{id}",
            delete(routes::sessions::revoke_session),
        )
        .route("/api/activity", get(routes::activity::list_activity))
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
            "/api/projects/{project}/members",
            get(routes::members::list_members),
        )
        .route(
            "/api/projects/{project}/members/{user_id}",
            put(routes::members::put_member).delete(routes::members::delete_member),
        )
        .route(
            "/api/projects/{project}/access-requests",
            get(routes::members::list_access_requests),
        )
        .route(
            "/api/projects/{project}/access-requests/{user_id}",
            delete(routes::members::deny_access_request),
        )
        .route(
            "/api/projects/{project}/access-request",
            post(routes::members::request_access).delete(routes::members::withdraw_access_request),
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
            "/api/projects/{project}/tasks/{id}/context",
            get(routes::context::get_context),
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
