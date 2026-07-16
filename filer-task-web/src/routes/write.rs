//! # Project Writes
//!
//! This module keeps each project mutation and its refreshed response inside
//! one lock. Reopening the project after taking the lock makes external task
//! edits visible to conditional writes and policy validation.

use filer_task::{
    agent_context::{ShowView, build_show},
    identity::TaskIdentity,
    model::Task,
    project::TaskProject,
};

use crate::{app::AppState, error::WebError, routes::blocking};

pub(crate) async fn mutate<F>(
    state: AppState,
    project_name: String,
    operation: F,
) -> Result<ShowView, WebError>
where
    F: FnOnce(&TaskProject, &[Task]) -> Result<TaskIdentity, WebError> + Send + 'static,
{
    let registered = state.registry.resolve(&project_name)?.clone();
    let write_lock = registered.write_lock();
    let _guard = write_lock.lock().await;
    blocking(move || {
        let project = registered.task_project()?.reload()?;
        let before = registered.validate_project(&project)?;
        let identity = operation(&project, &before.tasks)?;
        let after = registered.validate_project(&project)?;
        Ok(build_show(
            &project,
            &after.tasks,
            &identity,
            &after.warnings,
        )?)
    })
    .await
}
