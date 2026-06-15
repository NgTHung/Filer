//! # Routes
//!
//! Handlers stay thin: they resolve a root, then call `filer-task`. Business
//! rules (validation, criteria checks) live in the library, so the web layer
//! cannot drift from the CLI.

pub mod tasks;
pub mod transitions;

use filer_task::error::TaskError;

use crate::error::WebError;

/// Run synchronous `filer-task` work off the async runtime. Repo reads and file
/// writes would otherwise block a runtime thread for the duration of the call.
pub(crate) async fn blocking<F, T>(f: F) -> Result<T, WebError>
where
    F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|join| WebError::Task(TaskError::Message(format!("worker task failed: {join}"))))?
        .map_err(WebError::from)
}
