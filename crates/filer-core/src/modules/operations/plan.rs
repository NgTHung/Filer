//! # Operation plan
//!
//! Resolve Locations and validate native names before the queue takes ownership.
//! Provider preflight checks run again at execution because queued work can
//! outlive the filesystem state that passed admission.
//!
//! ```
//! use filer_core::{Command, Location, LocationRef, OperationId, RequestId};
//! use filer_core::model::session::SessionId;
//!
//! let create = Command::CreateFolder {
//!     parent: LocationRef::from_location(&Location::local("/tmp")),
//!     name: "example".into(),
//!     session: SessionId::DEFAULT,
//!     request: RequestId::new(),
//!     operation: OperationId::new(),
//! };
//! assert!(create.operation_id().is_some());
//! ```

use std::path::PathBuf;

use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::{Capabilities, CoreError, LocationRef, OperationId, OperationKind, RequestId};

use super::command::OpsCommand;
use super::target::{resolve_direct_target, resolve_direct_targets, resolve_named_target};

#[derive(Clone)]
pub(super) struct OperationScope {
    pub session: SessionId,
    pub request: RequestId,
    pub id: OperationId,
    pub kind: OperationKind,
}

pub(super) enum OperationPlan {
    Transfer {
        sources: Vec<PathBuf>,
        destination: PathBuf,
        moving: bool,
    },
    Delete {
        targets: Vec<(LocationRef, PathBuf)>,
        trash: bool,
    },
    Rename {
        source: PathBuf,
        target: PathBuf,
    },
    Create {
        parent: PathBuf,
        target: PathBuf,
        folder: bool,
    },
}

impl OperationPlan {
    pub fn resolve(
        command: OpsCommand,
        scope: &OperationScope,
        registry: &NodeRegistry,
        caps: Capabilities,
    ) -> Result<Self, CoreError> {
        let kind = &scope.kind;
        match command {
            OpsCommand::Copy {
                sources,
                destination,
                ..
            }
            | OpsCommand::Move {
                sources,
                destination,
                ..
            } => Ok(Self::Transfer {
                sources: resolve_direct_targets(registry, &sources, kind.clone(), caps)?,
                destination: resolve_direct_target(registry, &destination, kind.clone(), caps)?,
                moving: *kind == OperationKind::Move,
            }),
            OpsCommand::Delete { targets, trash, .. } => {
                let paths = resolve_direct_targets(registry, &targets, kind.clone(), caps)?;
                Ok(Self::Delete {
                    targets: targets.into_iter().zip(paths).collect(),
                    trash,
                })
            }
            OpsCommand::Rename {
                source, new_name, ..
            } => {
                let source =
                    resolve_named_target(registry, &source, &new_name, kind.clone(), caps)?;
                let parent = source.parent().ok_or_else(|| {
                    CoreError::invalid_path(format!("Cannot get parent of {}", source.display()))
                })?;
                let target = parent.join(new_name);
                Ok(Self::Rename { source, target })
            }
            OpsCommand::CreateFolder { parent, name, .. }
            | OpsCommand::CreateFile { parent, name, .. } => {
                let parent = resolve_named_target(registry, &parent, &name, kind.clone(), caps)?;
                let target = parent.join(name);
                Ok(Self::Create {
                    parent,
                    target,
                    folder: *kind == OperationKind::CreateFolder,
                })
            }
            OpsCommand::Cancel(_)
            | OpsCommand::Drain(..)
            | OpsCommand::CancelOperation { .. }
            | OpsCommand::InspectQueue { .. }
            | OpsCommand::ResolveQueue { .. } => {
                Err(CoreError::invalid_input("Expected a file operation"))
            }
        }
    }
}
