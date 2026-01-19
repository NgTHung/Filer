use std::path::PathBuf;

use crate::model::node::NodeId;
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
    },

    /// Scan progress update
    ScanProgress {
        scanned: usize,
        current: NodeId,
    },
    
    /// Batch of files (streaming results)
    FilesBatch(Vec<FileNode>),
    
    /// Search results
    SearchResults {
        query: String,
        matches: Vec<FileNode>,
        complete: bool,
    },
    
    /// Filesystem change detected
    FsChanged {
        node: NodeId,
        kind: FsChangeKind,
    },
    
    /// File operation completed
    OperationComplete {
        operation: OperationKind,
        success: bool,
        affected: Vec<NodeId>,
    },
    
    /// Error occurred
    Error {
        message: String,
        recoverable: bool,
    },
    
    /// Basic metadata loaded
    MetadataLoaded {
        node: NodeId,
        basic: BasicMetadata,
    },
    
    /// Extended metadata loaded
    ExtendedMetadataLoaded {
        node: NodeId,
        extended: ExtendedMetadata,
    },
    
    /// Preview ready
    PreviewReady {
        node: NodeId,
        preview: PreviewData,
    },
    
    /// Preview generation failed
    PreviewFailed {
        node: NodeId,
        reason: String,
    },
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
