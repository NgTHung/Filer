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
/// Location-native commands are preferred for new read-side provider-aware
/// work. `NodeId` commands remain supported compatibility surfaces for
/// direct-local flows, internal cache handles, selection, and future
/// capability-specific migrations.
#[derive(Clone)]
pub enum Command {
    /// Compatibility direct-local navigation by path.
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

    /// Compatibility direct-local navigation by `NodeId`.
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

    /// Compatibility direct-local search by `NodeId`.
    Search {
        query: String,
        root: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility direct-local search by path.
    SearchPath {
        query: String,
        root: PathBuf,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native search. Preferred for new provider-aware read clients.
    SearchLocation {
        query: String,
        root: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Cancel current operation
    Cancel(SessionId),

    /// Compatibility preview request by `NodeId`.
    LoadPreview {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },

    /// Hybrid preview request by `LocationRef`.
    ///
    /// The input is Location-native, but the current preview result events
    /// still carry `NodeId` until the preview result contract migrates.
    LoadPreviewLocation {
        location: LocationRef,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },

    /// Cancel preview generation
    CancelPreview(SessionId),

    /// Future provider-capability work: write routing is still `NodeId`-first.
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Future provider-capability work: write routing is still `NodeId`-first.
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Future provider-capability work: write routing is still `NodeId`-first.
    Delete {
        nodes: Vec<NodeId>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Future provider-capability work: write routing is still `NodeId`-first.
    Rename {
        node: NodeId,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Future provider-capability work: write routing is still `NodeId`-first.
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Future provider-capability work: write routing is still `NodeId`-first.
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },

    /// Compatibility metadata request by `NodeId`.
    LoadMetadata {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Hybrid metadata request by `LocationRef`.
    ///
    /// The input is Location-native, but the current metadata result event
    /// still carries `NodeId` until the metadata result contract migrates.
    LoadMetadataLocation {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility extended metadata request by `NodeId`.
    LoadExtendedMetadata {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },

    /// Hybrid extended metadata request by `LocationRef`.
    ///
    /// The input is Location-native, but the current extended metadata result
    /// event still carries `NodeId` until the metadata result contract migrates.
    LoadExtendedMetadataLocation {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Compatibility direct-local scan by path.
    Scan {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    /// Location-native directory scan. Preferred for new provider-aware listing.
    ScanLocation {
        location: LocationRef,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },

    /// Compatibility direct-local scan by `NodeId`.
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

    /// Future provider-capability work: watching is still `NodeId`-first.
    Watch(NodeId, SessionId),

    /// Future provider-capability work: watching is still `NodeId`-first.
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
