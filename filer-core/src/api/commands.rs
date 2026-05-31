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

/// Commands from UI to Core.
///
/// Location-native commands are the canonical public command surface.
/// Path and `NodeId` commands remain supported explicit compatibility surfaces
/// for direct-local flows, internal cache handles, selection, and future
/// capability-specific migrations.
#[derive(Clone)]
pub enum Command {
    /// Compatibility direct-local navigation by path.
    NavigatePathCompat {
        path: PathBuf,
        session: SessionId,
        request: RequestId,
    },

    /// Navigate to a provider-aware location.
    Navigate {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility direct-local navigation by `NodeId`.
    NavigateNodeCompat {
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

    /// Compatibility direct-local search by `NodeId`.
    SearchNodeCompat {
        query: String,
        root: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility direct-local search by path.
    SearchPathCompat {
        query: String,
        root: PathBuf,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native search. Preferred for new provider-aware read clients.
    Search {
        query: String,
        root: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Cancel active search work for this session.
    CancelSearch {
        session: SessionId,
    },

    /// Compatibility preview request by `NodeId`.
    LoadPreviewNodeCompat {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native preview request by `LocationRef`.
    LoadPreview {
        location: LocationRef,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },

    /// Cancel active preview work for this session.
    CancelPreview {
        session: SessionId,
    },

    /// Compatibility write operation by `NodeId`.
    CopyNodeCompat {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Location-native copy for direct-local locations.
    Copy {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility write operation by `NodeId`.
    MoveNodeCompat {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Location-native move for direct-local locations.
    Move {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility write operation by `NodeId`.
    DeleteNodeCompat {
        nodes: Vec<NodeId>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Location-native delete for direct-local locations.
    Delete {
        locations: Vec<LocationRef>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility write operation by `NodeId`.
    RenameNodeCompat {
        node: NodeId,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Location-native rename for direct-local locations.
    Rename {
        location: LocationRef,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility write operation by `NodeId`.
    CreateFolderNodeCompat {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Location-native folder creation for direct-local parent locations.
    CreateFolder {
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility write operation by `NodeId`.
    CreateFileNodeCompat {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Location-native file creation for direct-local parent locations.
    CreateFile {
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility metadata request by `NodeId`.
    LoadMetadataNodeCompat {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native metadata request by `LocationRef`.
    LoadMetadata {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility extended metadata request by `NodeId`.
    LoadExtendedMetadataNodeCompat {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native extended metadata request by `LocationRef`.
    LoadExtendedMetadata {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility direct-local scan by path.
    ScanPathCompat {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    /// Location-native directory scan. Preferred for new provider-aware listing.
    Scan {
        location: LocationRef,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    /// Compatibility direct-local scan by `NodeId`.
    ScanNodeCompat {
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

    /// Cancel an active scan for this session.
    CancelScan {
        session: SessionId,
    },

    /// Cancel a specific file operation for this session.
    CancelOperation {
        session: SessionId,
        operation: OperationId,
    },

    /// Compatibility watch by `NodeId`.
    WatchNodeCompat {
        node: NodeId,
        session: SessionId,
    },

    /// Location-native watch for direct-local locations.
    Watch {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility unwatch by `NodeId`.
    UnwatchNodeCompat {
        node: NodeId,
    },

    /// Location-native unwatch for a direct-local location.
    Unwatch {
        location: LocationRef,
        session: SessionId,
    },
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
    /// - `UnwatchNodeCompat` (operates on NodeId, not session-scoped)
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            Command::Handshake => None,
            Command::DestroySession(_) => None,
            Command::UnwatchNodeCompat { .. } => None,

            Command::NavigatePathCompat { session, .. }
            | Command::Navigate { session, .. }
            | Command::NavigateNodeCompat { session, .. }
            | Command::NavigateUp { session, .. }
            | Command::NavigateBack { session, .. }
            | Command::NavigateForward { session, .. }
            | Command::Refresh { session, .. }
            | Command::SearchNodeCompat { session, .. }
            | Command::SearchPathCompat { session, .. }
            | Command::Search { session, .. }
            | Command::LoadPreviewNodeCompat { session, .. }
            | Command::LoadPreview { session, .. }
            | Command::CopyNodeCompat { session, .. }
            | Command::Copy { session, .. }
            | Command::MoveNodeCompat { session, .. }
            | Command::Move { session, .. }
            | Command::DeleteNodeCompat { session, .. }
            | Command::Delete { session, .. }
            | Command::RenameNodeCompat { session, .. }
            | Command::Rename { session, .. }
            | Command::CreateFolderNodeCompat { session, .. }
            | Command::CreateFolder { session, .. }
            | Command::CreateFileNodeCompat { session, .. }
            | Command::CreateFile { session, .. }
            | Command::LoadMetadataNodeCompat { session, .. }
            | Command::LoadMetadata { session, .. }
            | Command::LoadExtendedMetadataNodeCompat { session, .. }
            | Command::LoadExtendedMetadata { session, .. }
            | Command::ScanPathCompat { session, .. }
            | Command::Scan { session, .. }
            | Command::ScanNodeCompat { session, .. }
            | Command::SetPipeline { session, .. }
            | Command::CancelSearch { session }
            | Command::CancelPreview { session }
            | Command::CancelScan { session }
            | Command::CancelOperation { session, .. }
            | Command::WatchNodeCompat { session, .. }
            | Command::Watch { session, .. }
            | Command::Unwatch { session, .. }
            | Command::UnwatchSession(session)
            | Command::Extension { session, .. } => Some(*session),
        }
    }

    /// Extract the request ID for commands that are tied to a UI/API request.
    pub fn request_id(&self) -> Option<RequestId> {
        match self {
            Command::NavigatePathCompat { request, .. }
            | Command::Navigate { request, .. }
            | Command::NavigateNodeCompat { request, .. }
            | Command::NavigateUp { request, .. }
            | Command::NavigateBack { request, .. }
            | Command::NavigateForward { request, .. }
            | Command::Refresh { request, .. }
            | Command::SearchNodeCompat { request, .. }
            | Command::SearchPathCompat { request, .. }
            | Command::Search { request, .. }
            | Command::LoadPreviewNodeCompat { request, .. }
            | Command::LoadPreview { request, .. }
            | Command::CopyNodeCompat { request, .. }
            | Command::Copy { request, .. }
            | Command::MoveNodeCompat { request, .. }
            | Command::Move { request, .. }
            | Command::DeleteNodeCompat { request, .. }
            | Command::Delete { request, .. }
            | Command::RenameNodeCompat { request, .. }
            | Command::Rename { request, .. }
            | Command::CreateFolderNodeCompat { request, .. }
            | Command::CreateFolder { request, .. }
            | Command::CreateFileNodeCompat { request, .. }
            | Command::CreateFile { request, .. }
            | Command::LoadMetadataNodeCompat { request, .. }
            | Command::LoadMetadata { request, .. }
            | Command::LoadExtendedMetadataNodeCompat { request, .. }
            | Command::LoadExtendedMetadata { request, .. }
            | Command::ScanPathCompat { request, .. }
            | Command::Scan { request, .. }
            | Command::ScanNodeCompat { request, .. }
            | Command::Watch { request, .. } => Some(*request),

            Command::CancelSearch { .. }
            | Command::CancelPreview { .. }
            | Command::SetPipeline { .. }
            | Command::CancelScan { .. }
            | Command::CancelOperation { .. }
            | Command::WatchNodeCompat { .. }
            | Command::UnwatchNodeCompat { .. }
            | Command::Unwatch { .. }
            | Command::UnwatchSession(_)
            | Command::Handshake
            | Command::DestroySession(_)
            | Command::Extension { .. } => None,
        }
    }

    /// Extract the operation ID for write-operation commands.
    pub fn operation_id(&self) -> Option<OperationId> {
        match self {
            Command::CopyNodeCompat { operation, .. }
            | Command::Copy { operation, .. }
            | Command::MoveNodeCompat { operation, .. }
            | Command::Move { operation, .. }
            | Command::DeleteNodeCompat { operation, .. }
            | Command::Delete { operation, .. }
            | Command::RenameNodeCompat { operation, .. }
            | Command::Rename { operation, .. }
            | Command::CreateFolderNodeCompat { operation, .. }
            | Command::CreateFolder { operation, .. }
            | Command::CreateFileNodeCompat { operation, .. }
            | Command::CreateFile { operation, .. }
            | Command::CancelOperation { operation, .. } => Some(*operation),
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
            Command::NavigatePathCompat { .. } => "navigate.path.compat",
            Command::Navigate { .. } => "navigate",
            Command::NavigateNodeCompat { .. } => "navigate.node.compat",
            Command::NavigateUp { .. } => "navigate.up",
            Command::NavigateBack { .. } => "navigate.back",
            Command::NavigateForward { .. } => "navigate.forward",
            Command::Refresh { .. } => "navigate.refresh",
            Command::SearchNodeCompat { .. } => "search.node.compat",
            Command::SearchPathCompat { .. } => "search.path.compat",
            Command::Search { .. } => "search",
            Command::CancelSearch { .. } => "search.cancel",
            Command::LoadPreviewNodeCompat { .. } => "preview.load.node.compat",
            Command::LoadPreview { .. } => "preview.load",
            Command::CancelPreview { .. } => "preview.cancel",
            Command::LoadMetadataNodeCompat { .. } => "metadata.load.node.compat",
            Command::LoadMetadata { .. } => "metadata.load",
            Command::LoadExtendedMetadataNodeCompat { .. } => "metadata.extended.node.compat",
            Command::LoadExtendedMetadata { .. } => "metadata.extended",
            Command::CopyNodeCompat { .. } => "ops.copy.node.compat",
            Command::Copy { .. } => "ops.copy",
            Command::MoveNodeCompat { .. } => "ops.move.node.compat",
            Command::Move { .. } => "ops.move",
            Command::DeleteNodeCompat { .. } => "ops.delete.node.compat",
            Command::Delete { .. } => "ops.delete",
            Command::RenameNodeCompat { .. } => "ops.rename.node.compat",
            Command::Rename { .. } => "ops.rename",
            Command::CreateFolderNodeCompat { .. } => "ops.create_folder.node.compat",
            Command::CreateFolder { .. } => "ops.create_folder",
            Command::CreateFileNodeCompat { .. } => "ops.create_file.node.compat",
            Command::CreateFile { .. } => "ops.create_file",
            Command::ScanPathCompat { .. } => "scan.path.compat",
            Command::Scan { .. } => "scan",
            Command::ScanNodeCompat { .. } => "scan.node.compat",
            Command::SetPipeline { .. } => "navigate.pipeline",
            Command::CancelScan { .. } => "scan.cancel",
            Command::CancelOperation { .. } => "ops.cancel",
            Command::WatchNodeCompat { .. } => "watch.node.compat",
            Command::Watch { .. } => "watch",
            Command::UnwatchNodeCompat { .. } => "watch.node.remove.compat",
            Command::Unwatch { .. } => "watch.remove",
            Command::UnwatchSession(..) => "watch.session_remove",
            Command::Handshake => "session.handshake",
            Command::DestroySession(..) => "session.destroy",
            Command::Extension { key, .. } => key.as_str(),
        }
    }
}
