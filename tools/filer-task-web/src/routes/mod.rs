//! # Routes
//!
//! Handlers stay thin: they resolve a root, then call `taskroot`. Business
//! rules (validation, criteria checks) live in the library, so the web layer
//! cannot drift from the CLI.

pub mod activity;
pub mod context;
pub mod identity;
pub mod policy;
pub mod projects;
pub mod sessions;
pub mod task_writes;
pub mod tasks;
pub mod transitions;
mod write;

use crate::error::WebError;

/// Run synchronous `taskroot` work off the async runtime. Repo reads and file
/// writes would otherwise block a runtime thread for the duration of the call.
pub(crate) async fn blocking<F, T>(f: F) -> Result<T, WebError>
where
    F: FnOnce() -> Result<T, WebError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|join| {
        WebError::Task(taskroot::error::TaskError::Message(format!(
            "worker task failed: {join}"
        )))
    })?
}
