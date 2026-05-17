use std::path::PathBuf;
use std::sync::Arc;

use crate::PreviewOptions;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::LocationRef;
use crate::model::node::NodeId;
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;

/// Commands from UI to Core
/// Uses NodeId for efficiency (8 bytes vs PathBuf's heap allocation)
/// Core resolves NodeId -> PathBuf via NodeRegistry
#[derive(Clone)]
pub enum Command {
    /// Navigate to path (initial navigation uses PathBuf)
    Navigate {
        path: PathBuf,
        session: SessionId,
        request: RequestId,
    },

    /// Navigate to a provider-aware location.
    NavigateLocation {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Navigate to a node by ID (after initial load)
    NavigateToNode {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Go up one directory
    NavigateUp {
        session: SessionId,
        request: RequestId,
    },

    /// Go back in history
    NavigateBack {
        session: SessionId,
        request: RequestId,
    },

    /// Go forward in history
    NavigateForward {
        session: SessionId,
        request: RequestId,
    },

    /// Refresh current directory
    Refresh {
        session: SessionId,
        request: RequestId,
    },

    /// Search for files
    Search {
        query: String,
        root: NodeId,
        session: SessionId,
        request: RequestId,
    },

    SearchPath {
        query: String,
        root: PathBuf,
        session: SessionId,
        request: RequestId,
    },

    SearchLocation {
        query: String,
        root: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Cancel current operation
    Cancel(SessionId),

    /// Load preview for a node
    LoadPreview {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },

    LoadPreviewLocation {
        location: LocationRef,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },

    /// Cancel preview generation
    CancelPreview(SessionId),

    /// Copy nodes to destination
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Move nodes to destination
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Delete nodes
    Delete {
        nodes: Vec<NodeId>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Rename a node
    Rename {
        node: NodeId,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Create folder in parent
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Create file in parent
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Load basic metadata
    LoadMetadata {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    LoadMetadataLocation {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Load extended metadata (EXIF, ID3, etc.)
    LoadExtendedMetadata {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    LoadExtendedMetadataLocation {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Scan a directory by path (initial scan, returns batched results)
    Scan {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    ScanLocation {
        location: LocationRef,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    /// Scan a directory by NodeId (re-scan after navigation)
    ScanNode {
        node: NodeId,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    /// Update the current navigation pipeline for future refreshes.
    SetPipeline {
        session: SessionId,
        config: PipelineConfig,
    },

    /// Cancel an active scan for this session
    CancelScan(SessionId),

    /// Watch a directory for changes
    Watch(NodeId, SessionId),

    /// Stop watching a directory
    Unwatch(NodeId),
    UnwatchSession(SessionId),

    Handshake,

    DestroySession(SessionId),

    /// Extension point for custom commands from modules/plugins.
    ///
    /// The router dispatches these to handlers registered by key.
    /// Use `Arc<dyn Any + Send + Sync>` for a clonable, type-erased payload.
    ///
    /// # Example
    /// ```ignore
    /// core.send(Command::Extension {
    ///     key: "git.status".into(),
    ///     payload: Arc::new(GitStatusRequest { repo: path }),
    ///     session: my_session,
    /// });
    /// ```
    Extension {
        key: String,
        payload: Arc<dyn std::any::Any + Send + Sync>,
        session: SessionId,
    },
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Extension { key, session, .. } => f
                .debug_struct("Extension")
                .field("key", key)
                .field("session", session)
                .finish(),
            other => write!(f, "Command::{}", other.key()),
        }
    }
}

impl Command {
    /// Extract the SessionId from any command variant.
    ///
    /// Returns `None` for commands that don't require session validation:
    /// - `Handshake` (creates a session)
    /// - `DestroySession` (tears down — safe to no-op if unknown)
    /// - `Unwatch` (operates on NodeId, not session-scoped)
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            Command::Handshake => None,
            Command::DestroySession(_) => None,
            Command::Unwatch(_) => None,

            Command::Navigate { session: s, .. }
            | Command::NavigateLocation { session: s, .. }
            | Command::NavigateToNode { session: s, .. }
            | Command::NavigateUp { session: s, .. }
            | Command::Refresh { session: s, .. }
            | Command::Cancel(s)
            | Command::CancelPreview(s)
            | Command::CancelScan(s)
            | Command::Watch(_, s)
            | Command::UnwatchSession(s)
            | Command::NavigateBack { session: s, .. }
            | Command::NavigateForward { session: s, .. } => Some(*s),

            Command::Search { session, .. }
            | Command::SearchPath { session, .. }
            | Command::SearchLocation { session, .. }
            | Command::Scan { session, .. }
            | Command::ScanLocation { session, .. }
            | Command::ScanNode { session, .. }
            | Command::SetPipeline { session, .. }
            | Command::LoadPreview { session, .. }
            | Command::LoadPreviewLocation { session, .. }
            | Command::LoadMetadata { session, .. }
            | Command::LoadMetadataLocation { session, .. }
            | Command::LoadExtendedMetadata { session, .. }
            | Command::LoadExtendedMetadataLocation { session, .. }
            | Command::Copy { session, .. }
            | Command::Move { session, .. }
            | Command::Delete { session, .. }
            | Command::Rename { session, .. }
            | Command::CreateFolder { session, .. }
            | Command::CreateFile { session, .. }
            | Command::Extension { session, .. } => Some(*session),
        }
    }

    /// Extract the request ID for commands that are tied to a UI/API request.
    pub fn request_id(&self) -> Option<RequestId> {
        match self {
            Command::Navigate { request, .. }
            | Command::NavigateLocation { request, .. }
            | Command::NavigateToNode { request, .. }
            | Command::NavigateUp { request, .. }
            | Command::NavigateBack { request, .. }
            | Command::NavigateForward { request, .. }
            | Command::Refresh { request, .. }
            | Command::Search { request, .. }
            | Command::SearchPath { request, .. }
            | Command::SearchLocation { request, .. }
            | Command::LoadPreview { request, .. }
            | Command::LoadPreviewLocation { request, .. }
            | Command::LoadMetadata { request, .. }
            | Command::LoadMetadataLocation { request, .. }
            | Command::LoadExtendedMetadata { request, .. }
            | Command::LoadExtendedMetadataLocation { request, .. }
            | Command::Scan { request, .. }
            | Command::ScanLocation { request, .. }
            | Command::ScanNode { request, .. }
            | Command::Copy { request, .. }
            | Command::Move { request, .. }
            | Command::Delete { request, .. }
            | Command::Rename { request, .. }
            | Command::CreateFolder { request, .. }
            | Command::CreateFile { request, .. } => Some(*request),

            Command::Cancel(_)
            | Command::CancelPreview(_)
            | Command::SetPipeline { .. }
            | Command::CancelScan(_)
            | Command::Watch(_, _)
            | Command::Unwatch(_)
            | Command::UnwatchSession(_)
            | Command::Handshake
            | Command::DestroySession(_)
            | Command::Extension { .. } => None,
        }
    }

    /// Extract the operation ID for write-operation commands.
    pub fn operation_id(&self) -> Option<OperationId> {
        match self {
            Command::Copy { operation, .. }
            | Command::Move { operation, .. }
            | Command::Delete { operation, .. }
            | Command::Rename { operation, .. }
            | Command::CreateFolder { operation, .. }
            | Command::CreateFile { operation, .. } => Some(*operation),
            _ => None,
        }
    }

    /// Get the dispatch key for this command.
    ///
    /// The [`CommandRouter`] uses this to look up the registered handler.
    /// Core command variants return static keys; [`Extension`](Command::Extension)
    /// returns the user-provided key.
    pub fn key(&self) -> &str {
        match self {
            Command::Navigate { .. } => "navigate",
            Command::NavigateLocation { .. } => "navigate.location",
            Command::NavigateToNode { .. } => "navigate.node",
            Command::NavigateUp { .. } => "navigate.up",
            Command::NavigateBack { .. } => "navigate.back",
            Command::NavigateForward { .. } => "navigate.forward",
            Command::Refresh { .. } => "navigate.refresh",
            Command::Search { .. } => "search",
            Command::SearchPath { .. } => "search.path",
            Command::SearchLocation { .. } => "search.location",
            Command::Cancel(..) => "search.cancel",
            Command::LoadPreview { .. } => "preview.load",
            Command::LoadPreviewLocation { .. } => "preview.load.location",
            Command::CancelPreview(..) => "preview.cancel",
            Command::LoadMetadata { .. } => "metadata.load",
            Command::LoadMetadataLocation { .. } => "metadata.load.location",
            Command::LoadExtendedMetadata { .. } => "metadata.extended",
            Command::LoadExtendedMetadataLocation { .. } => "metadata.extended.location",
            Command::Copy { .. } => "ops.copy",
            Command::Move { .. } => "ops.move",
            Command::Delete { .. } => "ops.delete",
            Command::Rename { .. } => "ops.rename",
            Command::CreateFolder { .. } => "ops.create_folder",
            Command::CreateFile { .. } => "ops.create_file",
            Command::Scan { .. } => "scan",
            Command::ScanLocation { .. } => "scan.location",
            Command::ScanNode { .. } => "scan.node",
            Command::SetPipeline { .. } => "navigate.pipeline",
            Command::CancelScan(..) => "scan.cancel",
            Command::Watch(..) => "watch",
            Command::Unwatch(..) => "watch.remove",
            Command::UnwatchSession(..) => "watch.session_remove",
            Command::Handshake => "session.handshake",
            Command::DestroySession(..) => "session.destroy",
            Command::Extension { key, .. } => key.as_str(),
        }
    }
}
