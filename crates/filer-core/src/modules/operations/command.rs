//! # Operation commands
//!
//! This module defines the messages accepted by the operation actor. The actor
//! re-exports these types so existing callers keep one stable import path.
//!
//! ```
//! use filer_core::modules::operations::operator::{OperationEventMode, OpsCommand};
//! use filer_core::model::session::SessionId;
//! use filer_core::OperationId;
//!
//! let command = OpsCommand::CancelOperation {
//!     session: SessionId(1),
//!     operation: OperationId(2),
//! };
//! assert!(matches!(command, OpsCommand::CancelOperation { .. }));
//! let mode = OperationEventMode::Location;
//! assert_eq!(mode, OperationEventMode::Location);
//! ```

use crate::model::location::LocationRef;
use crate::model::operation::{OperationId, OperationKind};

use super::plan::OperationScope;
use crate::model::request::RequestId;
use crate::model::session::SessionId;

#[derive(Debug, Clone)]
pub enum OpsCommand {
    Copy {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Move {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Delete {
        targets: Vec<LocationRef>,
        trash: bool,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Rename {
        source: LocationRef,
        new_name: String,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFolder {
        parent: LocationRef,
        name: String,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFile {
        parent: LocationRef,
        name: String,
        event_mode: OperationEventMode,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Cancel(SessionId),
    CancelOperation {
        session: SessionId,
        operation: OperationId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationEventMode {
    Location,
}

impl OpsCommand {
    pub(super) fn scope(&self) -> Option<OperationScope> {
        use OperationKind::*;
        let (session, request, id, kind) = match *self {
            Self::Copy {
                session,
                request,
                operation,
                ..
            } => (session, request, operation, Copy),
            Self::Move {
                session,
                request,
                operation,
                ..
            } => (session, request, operation, Move),
            Self::Delete {
                session,
                request,
                operation,
                ..
            } => (session, request, operation, Delete),
            Self::Rename {
                session,
                request,
                operation,
                ..
            } => (session, request, operation, Rename),
            Self::CreateFile {
                session,
                request,
                operation,
                ..
            } => (session, request, operation, CreateFile),
            Self::CreateFolder {
                session,
                request,
                operation,
                ..
            } => (session, request, operation, CreateFolder),
            Self::Cancel(_) | Self::CancelOperation { .. } => return None,
        };
        Some(OperationScope {
            session,
            request,
            id,
            kind,
        })
    }
}
