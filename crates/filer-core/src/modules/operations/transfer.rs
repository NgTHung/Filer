//! # Transfer operations
//!
//! This module runs copy and move work outside the actor dispatch loop. Both
//! handlers use the shared provider context, event, and cache lifecycle.
//!
//! ```
//! use filer_core::model::session::SessionId;
//! use filer_core::modules::operations::operator::{OperationEventMode, OpsCommand};
//! use filer_core::{Location, LocationRef, OperationId, RequestId};
//!
//! let source = LocationRef::from_location(&Location::local("/tmp/source"));
//! let destination = LocationRef::from_location(&Location::local("/tmp/destination"));
//! let command = OpsCommand::Copy {
//!     sources: vec![source],
//!     destination,
//!     event_mode: OperationEventMode::Location,
//!     session: SessionId(1),
//!     request: RequestId(2),
//!     operation: OperationId(3),
//! };
//! assert!(matches!(command, OpsCommand::Copy { .. }));
//! ```

use std::path::{Path, PathBuf};

use crate::{CoreError, Location, LocationRef, ProgressStatus};

use super::execution::Execution;
use super::preflight::validated_targets;
use super::support::is_cross_device;

impl Execution<'_> {
    pub(super) async fn transfer(
        &mut self,
        sources: &[PathBuf],
        destination: &Path,
        moving: bool,
    ) -> Result<(), CoreError> {
        let fs = self.provider();
        let targets = validated_targets(fs, sources, destination, &self.cx).await?;
        for (source, target) in sources.iter().zip(targets) {
            if moving {
                self.move_target(source, target).await?;
            } else {
                self.copy_target(source, target).await?;
            }
        }
        Ok(())
    }

    async fn move_target(&mut self, source: &Path, target: PathBuf) -> Result<(), CoreError> {
        let fs = self.provider();
        match self.write(fs.rename(source, &target, &self.cx)).await {
            Ok(()) => self.record_target(target),
            Err(error) if is_cross_device(&error) => {
                self.write(fs.copy(source, &target, &self.cx)).await?;
                self.record_target(target);
                self.write(self.provider().delete(source, &self.cx)).await?;
            }
            Err(error) => return Err(error),
        }
        self.invalidate_removed(source);
        self.done += 1;
        Ok(())
    }

    async fn copy_target(&mut self, source: &Path, target: PathBuf) -> Result<(), CoreError> {
        let fs = self.provider();
        let meta = self
            .cx
            .race(fs.scheme(), fs.metadata(source, &self.cx))
            .await?;
        if meta.is_dir() {
            self.write(fs.mkdir(&target, &self.cx)).await?;
            self.record_target(target.clone());
            self.copy_children(source, &target).await?;
        } else {
            self.write(fs.copy(source, &target, &self.cx)).await?;
            self.record_target(target);
            self.done += 1;
        }
        Ok(())
    }

    async fn copy_children(&mut self, source: &Path, target: &Path) -> Result<(), CoreError> {
        let fs = self.provider();
        let entries = self.cx.race(fs.scheme(), fs.list(source, &self.cx)).await?;
        for entry in entries {
            let source = source.join(&entry.name);
            let target = target.join(&entry.name);
            let fs = self.provider();
            if entry.is_dir() {
                self.write(fs.mkdir(&target, &self.cx)).await?;
                Box::pin(self.copy_children(&source, &target)).await?;
            } else {
                self.write(fs.copy(&source, &target, &self.cx)).await?;
                self.done += 1;
                let current = LocationRef::from_location(&Location::local(target));
                self.progress(ProgressStatus::Running, Some(current)).await;
            }
        }
        Ok(())
    }
}
