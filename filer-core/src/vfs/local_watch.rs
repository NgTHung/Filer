use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind, RenameMode};
use notify::{Config as NotifyConfig, Event as NotifyEvent, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, NoCache, new_debouncer_opt};
use std::time::Duration;

use crate::errors::CoreError;
use crate::model::fs_change::FsChangeKind;
use crate::vfs::watch::{FsChange, WatchHandle, WatchProvider};

/// Watch handle for the local filesystem — wraps the `notify` debouncer.
struct LocalWatchHandle;

impl WatchHandle for LocalWatchHandle {}

/// Local filesystem watch provider backed by [`notify`].
///
/// Uses OS-native mechanisms (inotify on Linux, FSEvents on macOS,
/// ReadDirectoryChangesW on Windows) with debouncing.
pub struct LocalWatchProvider {
    debouncer: Mutex<Option<Debouncer<RecommendedWatcher, NoCache>>>,
}

impl LocalWatchProvider {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {
            debouncer: Mutex::new(None),
        }
    }
    #[allow(unused)]
    /// Ensure the debouncer is initialised, lazily creating it on first watch.
    fn ensure_debouncer(&self, tx: &flume::Sender<FsChange>) -> Result<(), CoreError> {
        let mut guard = self.debouncer.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }

        let tx = tx.clone();
        let debouncer = new_debouncer_opt(
            Duration::from_secs(1),
            Some(Duration::from_millis(100)),
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for debounced in events {
                        let changes = notify_to_changes(&debounced.event);
                        for change in changes {
                            let _ = tx.send(change);
                        }
                    }
                }
                Err(errors) => {
                    for error in errors {
                        tracing::error!("notify watch error: {:?}", error);
                    }
                }
            },
            NoCache,
            NotifyConfig::default(),
        )
        .map_err(|e| {
            CoreError::io(
                PathBuf::new(),
                format!("failed to create notify debouncer: {e}"),
            )
        })?;

        *guard = Some(debouncer);
        Ok(())
    }
}

#[async_trait]
impl WatchProvider for LocalWatchProvider {
    async fn watch(
        &self,
        path: &Path,
        tx: flume::Sender<FsChange>,
    ) -> Result<Box<dyn WatchHandle>, CoreError> {
        self.ensure_debouncer(&tx)?;

        let mut guard = self.debouncer.lock().unwrap();
        let debouncer = guard.as_mut().unwrap();

        debouncer
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| CoreError::io(path.to_path_buf(), format!("notify watch failed: {e}")))?;

        Ok(Box::new(LocalWatchHandle))
    }

    async fn unwatch(&self, path: &Path) -> Result<(), CoreError> {
        let mut guard = self.debouncer.lock().unwrap();
        if let Some(debouncer) = guard.as_mut() {
            debouncer.unwatch(path).map_err(|e| {
                CoreError::io(path.to_path_buf(), format!("notify unwatch failed: {e}"))
            })?;
        }
        Ok(())
    }
}

/// Convert a single `notify` event into zero or more [`FsChange`]s.
#[allow(unused)]
fn notify_to_changes(event: &NotifyEvent) -> Vec<FsChange> {
    let kind = match &event.kind {
        EventKind::Create(CreateKind::File)
        | EventKind::Create(CreateKind::Folder)
        | EventKind::Create(CreateKind::Any) => Some(FsChangeKind::Created),

        EventKind::Modify(ModifyKind::Data(_))
        | EventKind::Modify(ModifyKind::Metadata(_))
        | EventKind::Modify(ModifyKind::Any) => Some(FsChangeKind::Modified),

        EventKind::Remove(RemoveKind::File)
        | EventKind::Remove(RemoveKind::Folder)
        | EventKind::Remove(RemoveKind::Any) => Some(FsChangeKind::Deleted),

        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => Some(FsChangeKind::Created),

        _ => None,
    };

    // Special case: RenameMode::From produces a Renamed with the source path
    if matches!(
        &event.kind,
        EventKind::Modify(ModifyKind::Name(RenameMode::From))
    ) {
        return event
            .paths
            .iter()
            .map(|p| FsChange {
                path: p.clone(),
                kind: FsChangeKind::Renamed { from: p.clone() },
            })
            .collect();
    }

    match kind {
        Some(k) => event
            .paths
            .iter()
            .map(|p| FsChange {
                path: p.clone(),
                kind: k.clone(),
            })
            .collect(),
        None => Vec::new(),
    }
}
