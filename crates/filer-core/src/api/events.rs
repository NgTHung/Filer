use crate::errors::{CoreError, ErrorCode, ErrorContext, ErrorKind, ErrorTarget};
use crate::model::directory::{DirectoryLoadState, DirectoryPageState};
use crate::model::location::LocationRef;
use crate::model::node::{NodeEntry, NodeMeta};
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{ProgressScope, ProgressSnapshot};
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::git_decorations::{FileDecoration, FileDecorationInvalidation};
use crate::modules::navigation::navigator::NavState;
use crate::pipeline::GroupedEntries;
use crate::{ExtendedMetadata, PreviewData, model::fs_change::FsChangeKind};

/// Events from Core to UI.
///
/// Read events use provider-aware locations as their sole addressing contract.
#[derive(Debug, Clone)]
pub enum Event {
    /// Location-native directory contents loaded with provider-aware locations.
    DirectoryLoaded {
        parent: LocationRef,
        groups: GroupedEntries,
        load: DirectoryLoadState,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native directory page loaded with provider-aware locations.
    DirectoryPageLoaded {
        parent: LocationRef,
        groups: GroupedEntries,
        page: DirectoryPageState,
        session: SessionId,
        request: RequestId,
    },

    /// Generic progress update for scan, operation, and future long-running tasks.
    ProgressUpdated {
        scope: ProgressScope,
        snapshot: ProgressSnapshot,
    },

    /// Location-native search results by `NodeEntry`.
    SearchResults {
        matches: Vec<NodeEntry>,
        complete: bool,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native filesystem change.
    FsChanged {
        location: LocationRef,
        kind: FsChangeKind,
        session: SessionId,
    },

    /// Location-native operation completion by affected locations.
    OperationComplete {
        operation_id: OperationId,
        operation: OperationKind,
        success: bool,
        affected: Vec<LocationRef>,
        session: SessionId,
    },

    /// Error occurred
    Error {
        kind: ErrorKind,
        code: ErrorCode,
        target: Option<ErrorTarget>,
        context: Option<Box<ErrorContext>>,
        message: String,
        recoverable: bool,
        session: SessionId,
        request: Option<RequestId>,
        operation: Option<OperationId>,
    },

    /// Location-native metadata result.
    MetadataLoaded {
        location: LocationRef,
        meta: NodeMeta,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native extended metadata result.
    ExtendedMetadataLoaded {
        location: LocationRef,
        extended: ExtendedMetadata,
        session: SessionId,
        request: RequestId,
    },

    /// Semantic file decorations produced by an in-process extension.
    FileDecorationsUpdated {
        decorations: Vec<FileDecoration>,
        session: SessionId,
        request: RequestId,
    },

    /// Locations whose previously emitted decorations are no longer current.
    FileDecorationsInvalidated {
        invalidation: FileDecorationInvalidation,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native preview result.
    PreviewReady {
        location: LocationRef,
        preview: PreviewData,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native preview failure.
    PreviewFailed {
        location: LocationRef,
        reason: String,
        session: SessionId,
        request: RequestId,
    },

    SessionCreated(SessionId),

    SessionDestroyed(SessionId),

    CurrentNavigateState {
        session: SessionId,
        state: NavState,
    },
}

impl Event {
    /// Create an `Event::Error` from a [`CoreError`] and session.
    ///
    pub fn from_error(err: CoreError, session: SessionId) -> Self {
        err.emit_trace();
        Event::Error {
            kind: err.kind(),
            code: err.code(),
            target: err.target().cloned(),
            context: err.context().cloned().map(Box::new),
            message: err.to_string(),
            recoverable: err.recoverable(),
            session,
            request: None,
            operation: None,
        }
    }

    pub fn from_request_error(err: CoreError, session: SessionId, request: RequestId) -> Self {
        let mut event = Self::from_error(err, session);
        if let Event::Error { request: r, .. } = &mut event {
            *r = Some(request);
        }
        event
    }

    pub fn from_operation_error(
        err: CoreError,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    ) -> Self {
        let mut event = Self::from_request_error(err, session, request);
        if let Event::Error { operation: op, .. } = &mut event {
            *op = Some(operation);
        }
        event
    }
}
