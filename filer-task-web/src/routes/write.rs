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

use crate::{
    app::AppState,
    error::WebError,
    identity::Actor,
    routes::blocking,
    storage::NewActivity,
};

pub(crate) async fn mutate<F>(
    state: AppState,
    actor: Actor,
    project_name: String,
    action: &'static str,
    detail: Option<String>,
    operation: F,
) -> Result<ShowView, WebError>
where
    F: FnOnce(&TaskProject, &[Task]) -> Result<TaskIdentity, WebError> + Send + 'static,
{
    let registered = state.registry.resolve(&project_name)?.clone();
    let write_lock = registered.write_lock();
    let _guard = write_lock.lock().await;
    let view = blocking(move || {
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
    .await?;
    let task_id = view.detail.task.qualified_id();
    state
        .storage()
        .record_committed_activity(NewActivity {
            user_id: actor.user_id,
            username: &actor.username,
            project: &project_name,
            task_id: Some(&task_id),
            action,
            detail: detail.as_deref(),
        })
        .await;
    Ok(view)
}
