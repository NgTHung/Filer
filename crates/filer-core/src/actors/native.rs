//! # Native work lifetime
//!
//! A blocking job holds the same completion scope as its caller. Dropping the
//! caller's future leaves the job counted until its native closure releases it.
//!
//! ```
//! use filer_core::LocalFs;
//! assert!(filer_core::FsProvider::capabilities(&LocalFs::new()).read);
//! ```

use std::future::Future;
use std::io::{self, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::Notify;
use tokio::task::{JoinError, JoinHandle};

use crate::CoreError;

#[derive(Clone, Debug, Default)]
pub(crate) struct Scope(Arc<State>);

#[derive(Debug, Default)]
struct State {
    active: AtomicUsize,
    idle: Notify,
}

pub(crate) struct Permit(Scope);

tokio::task_local! {
    static CURRENT: Scope;
}

impl Scope {
    pub(crate) fn enter(&self) -> Permit {
        self.0.active.fetch_add(1, Ordering::AcqRel);
        Permit(self.clone())
    }

    pub(crate) async fn wait(&self) {
        loop {
            let notified = self.0.idle.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.0.active.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

impl Permit {
    pub(crate) async fn run<F: Future>(self, future: F) -> F::Output {
        CURRENT.scope(self.0.clone(), future).await
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        if self.0.0.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.0.idle.notify_waiters();
        }
    }
}

pub(crate) fn spawn_blocking<F, T>(work: F) -> impl Future<Output = Result<T, JoinError>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let permit = CURRENT.try_with(Scope::enter).ok();
    let task = tokio::task::spawn_blocking(move || (work(), permit));
    async move { task.await.map(|(value, _permit)| value) }
}

pub(crate) fn spawn_owned<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let permit = CURRENT.try_with(Scope::enter).ok();
    tokio::spawn(async move {
        match permit {
            Some(permit) => permit.run(future).await,
            None => future.await,
        }
    })
}

pub(crate) async fn file_io<T: Send + 'static>(
    path: &Path,
    work: impl FnOnce(&Path) -> io::Result<T> + Send + 'static,
) -> Result<T, CoreError> {
    let path = path.to_path_buf();
    spawn_blocking(move || work(&path).map_err(|error| CoreError::from_io_error(error, path)))
        .await
        .map_err(|error| CoreError::actor("filesystem", error.to_string()))?
}

pub(crate) fn read_once(file: &mut impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; limit];
    let count = file.read(&mut bytes)?;
    bytes.truncate(count);
    Ok(bytes)
}
