//! # Mutation operations
//!
//! Delete, rename, and create share one worker so cancellation and cache
//! invalidation follow the same rules across mutation types.
//!
//! ```
//! use filer_core::model::session::SessionId;
//! use filer_core::modules::operations::operator::{OperationEventMode, OpsCommand};
//! use filer_core::{Location, LocationRef, OperationId, RequestId};
//!
//! let target = LocationRef::from_location(&Location::local("/tmp/example"));
//! let command = OpsCommand::Delete {
//!     targets: vec![target],
//!     trash: false,
//!     event_mode: OperationEventMode::Location,
//!     session: SessionId(1),
//!     request: RequestId(2),
//!     operation: OperationId(3),
//! };
//! assert!(matches!(command, OpsCommand::Delete { .. }));
//! ```

use std::path::{Path, PathBuf};

use crate::{CoreError, LocationRef, ProgressStatus};

use super::execution::Execution;

impl Execution<'_> {
    pub(super) async fn delete(
        &mut self,
        targets: &[(LocationRef, PathBuf)],
        trash: bool,
    ) -> Result<(), CoreError> {
        self.set_total(targets.len());
        for (location, path) in targets {
            if self.cx.is_cancelled() {
                return Err(CoreError::cancelled());
            }
            self.destructive_attempted = true;
            if trash {
                let trash = self.executor.trash.clone();
                let target = path.clone();
                crate::actors::native::spawn_blocking(move || trash(&target))
                    .await
                    .map_err(|error| CoreError::actor("operator", error.to_string()))??;
            } else {
                self.write(self.provider().delete(path, &self.cx)).await?;
            }
            self.invalidate_removed(path);
            self.affected.push(location.clone());
            self.done += 1;
            if targets.len() > 1 {
                self.progress(ProgressStatus::Running, Some(location.clone()))
                    .await;
            }
        }
        Ok(())
    }

    pub(super) async fn rename(&mut self, source: &Path, target: &Path) -> Result<(), CoreError> {
        self.require_missing(source, target).await?;
        self.destructive_attempted = true;
        self.write(self.provider().rename(source, target, &self.cx))
            .await?;
        self.invalidate_removed(source);
        self.finish_target(target);
        Ok(())
    }

    pub(super) async fn create(
        &mut self,
        parent: &Path,
        target: &Path,
        folder: bool,
    ) -> Result<(), CoreError> {
        self.require_missing(parent, target).await?;
        let fs = self.provider();
        if folder {
            self.write(fs.mkdir(target, &self.cx)).await?;
        } else {
            self.write(fs.write(target, &[], &self.cx)).await?;
        }
        self.finish_target(target);
        Ok(())
    }

    fn finish_target(&mut self, target: &Path) {
        self.record_target(target.to_path_buf());
        self.done = 1;
        self.set_total(1);
    }
}
