use std::path::PathBuf;

use crate::errors::CoreError;
use crate::modules::navigation::navigator::NavState;
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::pipeline::GroupedNodes;
use crate::{BasicMetadata, ExtendedMetadata, FileNode, PreviewData, model::fs_change::FsChangeKind};

/// Events from Core to UI
/// FileNode contains full data for batches (UI caches these)
/// NodeId used for single-file references (UI looks up from cache)
#[derive(Debug, Clone)]
pub enum Event {
    /// Directory contents loaded (full data for UI to cache)
    ///
    /// Always carries `GroupedNodes`. When no grouping is configured,
    /// contains a single group with an empty label (degenerate flat list).
    /// The UI iterates `.groups` uniformly — one unnamed group renders
    /// as a flat list, multiple named groups render section headers.
    DirectoryLoaded {
        parent: NodeId,
        path: PathBuf,  // Keep path for display in breadcrumb
        groups: GroupedNodes,
        session: SessionId
    },
    
    /// Scan progress update
    ScanProgress {
        scanned: usize,
        current: NodeId,
        session: SessionId
    },
    
    /// Batch of files (streaming results)
    FilesBatch(Vec<FileNode>,SessionId),
    
    /// Search results
    SearchResults {
        matches: Vec<FileNode>,
        complete: bool,
        session: SessionId
    },
    
    /// Filesystem change detected
    FsChanged {
        node: NodeId,
        kind: FsChangeKind,
        session: SessionId
    },
    
    /// File operation completed
    OperationComplete {
        operation: OperationKind,
        success: bool,
        affected: Vec<NodeId>,
        session: SessionId
    },
    
    /// Error occurred
    Error {
        message: String,
        recoverable: bool,
        session: SessionId
    },
    
    /// Basic metadata loaded
    MetadataLoaded {
        node: NodeId,
        basic: BasicMetadata,
        session: SessionId
    },
    
    /// Extended metadata loaded
    ExtendedMetadataLoaded {
        node: NodeId,
        extended: ExtendedMetadata,
        session: SessionId
    },
    
    /// Preview ready
    PreviewReady {
        node: NodeId,
        preview: PreviewData,
        session: SessionId
    },
    
    /// Preview generation failed
    PreviewFailed {
        node: NodeId,
        reason: String,
        session: SessionId
    },

    SessionCreated(SessionId),

    SessionDestroyed(SessionId),

    CurrentNavigateState{
        session: SessionId,
        state: NavState
    }
}

impl Event {
    /// Create an `Event::Error` from a [`CoreError`] and session.
    ///
    /// Determines `recoverable` based on the error variant:
    /// - Recoverable: NotFound, PermissionDenied, InvalidPath, Cancelled, NetworkError
    /// - Non-recoverable: Io, ChannelClosed, ActorError, InvalidData, InvalidInput, Other
    pub fn from_error(err: CoreError, session: SessionId) -> Self {
        let recoverable = matches!(
            err,
            CoreError::NotFound(_)
                | CoreError::PermissionDenied(_)
                | CoreError::InvalidPath(_)
                | CoreError::Cancelled
                | CoreError::NetworkError(_)
        );
        Event::Error {
            message: err.to_string(),
            recoverable,
            session,
        }
    }
}

#[derive(Clone, Debug)]
pub enum OperationKind {
    Copy,
    Move,
    Delete,
    Rename,
    CreateFolder,
    CreateFile,
}
