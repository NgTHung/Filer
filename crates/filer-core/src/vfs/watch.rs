use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::fs_change::FsChangeKind;

/// A single filesystem change event from a watch provider.
#[derive(Debug, Clone)]
pub struct FsChange {
    pub path: PathBuf,
    pub kind: FsChangeKind,
}

/// Handle returned by [`WatchProvider::watch`].
///
/// Dropping the handle stops watching. Providers implement [`Drop`]
/// on their concrete handle type to clean up OS resources.
pub trait WatchHandle: Send + Sync + 'static {}

/// Trait for filesystem watch backends.
///
/// Each VFS backend that supports watching implements this trait.
/// The [`Watcher`](crate::modules::watch::watcher::Watcher) actor
/// delegates to the provider instead of hard-coding `notify`.
#[async_trait]
pub trait WatchProvider: Send + Sync + 'static {
    /// Start watching `path` recursively.
    ///
    /// Change events are sent to `tx`. Returns a handle whose drop
    /// stops the watch.
    async fn watch(
        &self,
        path: &Path,
        tx: flume::Sender<FsChange>,
    ) -> Result<Box<dyn WatchHandle>, CoreError>;

    /// Stop watching `path`.
    async fn unwatch(&self, path: &Path) -> Result<(), CoreError>;
}
