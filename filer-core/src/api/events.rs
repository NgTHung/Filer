use std::path::PathBuf;

use crate::actors::navigator::NavState;
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::{BasicMetadata, ExtendedMetadata, FileNode, PreviewData, model::fs_change::FsChangeKind};

/// Events from Core to UI
/// FileNode contains full data for batches (UI caches these)
/// NodeId used for single-file references (UI looks up from cache)
#[derive(Debug, Clone)]
pub enum Event {
    /// Directory contents loaded (full data for UI to cache)
    DirectoryLoaded {
        parent: NodeId,
        path: PathBuf,  // Keep path for display in breadcrumb
        entries: Vec<FileNode>,
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
        query: String,
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

#[derive(Clone, Debug)]
pub enum OperationKind {
    Copy,
    Move,
    Delete,
    Rename,
    CreateFolder,
    CreateFile,
}
