use std::sync::Arc;

use crate::PreviewOptions;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::LocationRef;
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;

/// Commands from UI to Core.
///
/// Location-native commands are the canonical public command surface.
#[derive(Clone)]
pub enum Command {
    /// Navigate to a provider-aware location.
    Navigate {
        location: LocationRef,
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

    /// Location-native copy for direct-local locations.
    Copy {
        sources: Vec<LocationRef>,
        destination: LocationRef,
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

    /// Location-native delete for direct-local locations.
    Delete {
        locations: Vec<LocationRef>,
        trash: bool,
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

    /// Location-native folder creation for direct-local parent locations.
    CreateFolder {
        parent: LocationRef,
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

    /// Location-native metadata request by `LocationRef`.
    LoadMetadata {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },

    /// Location-native extended metadata request by `LocationRef`.
    LoadExtendedMetadata {
        location: LocationRef,
        session: SessionId,
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

    /// Location-native watch for direct-local locations.
    Watch {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
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
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            Command::Handshake => None,
            Command::DestroySession(_) => None,

            Command::Navigate { session, .. }
            | Command::NavigateUp { session, .. }
            | Command::NavigateBack { session, .. }
            | Command::NavigateForward { session, .. }
            | Command::Refresh { session, .. }
            | Command::Search { session, .. }
            | Command::LoadPreview { session, .. }
            | Command::Copy { session, .. }
            | Command::Move { session, .. }
            | Command::Delete { session, .. }
            | Command::Rename { session, .. }
            | Command::CreateFolder { session, .. }
            | Command::CreateFile { session, .. }
            | Command::LoadMetadata { session, .. }
            | Command::LoadExtendedMetadata { session, .. }
            | Command::Scan { session, .. }
            | Command::SetPipeline { session, .. }
            | Command::CancelSearch { session }
            | Command::CancelPreview { session }
            | Command::CancelScan { session }
            | Command::CancelOperation { session, .. }
            | Command::Watch { session, .. }
            | Command::Unwatch { session, .. }
            | Command::UnwatchSession(session)
            | Command::Extension { session, .. } => Some(*session),
        }
    }

    /// Extract the request ID for commands that are tied to a UI/API request.
    pub fn request_id(&self) -> Option<RequestId> {
        match self {
            Command::Navigate { request, .. }
            | Command::NavigateUp { request, .. }
            | Command::NavigateBack { request, .. }
            | Command::NavigateForward { request, .. }
            | Command::Refresh { request, .. }
            | Command::Search { request, .. }
            | Command::LoadPreview { request, .. }
            | Command::Copy { request, .. }
            | Command::Move { request, .. }
            | Command::Delete { request, .. }
            | Command::Rename { request, .. }
            | Command::CreateFolder { request, .. }
            | Command::CreateFile { request, .. }
            | Command::LoadMetadata { request, .. }
            | Command::LoadExtendedMetadata { request, .. }
            | Command::Scan { request, .. }
            | Command::Watch { request, .. } => Some(*request),

            Command::CancelSearch { .. }
            | Command::CancelPreview { .. }
            | Command::SetPipeline { .. }
            | Command::CancelScan { .. }
            | Command::CancelOperation { .. }
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
            Command::Copy { operation, .. }
            | Command::Move { operation, .. }
            | Command::Delete { operation, .. }
            | Command::Rename { operation, .. }
            | Command::CreateFolder { operation, .. }
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
            Command::Navigate { .. } => "navigate",
            Command::NavigateUp { .. } => "navigate.up",
            Command::NavigateBack { .. } => "navigate.back",
            Command::NavigateForward { .. } => "navigate.forward",
            Command::Refresh { .. } => "navigate.refresh",
            Command::Search { .. } => "search",
            Command::CancelSearch { .. } => "search.cancel",
            Command::LoadPreview { .. } => "preview.load",
            Command::CancelPreview { .. } => "preview.cancel",
            Command::LoadMetadata { .. } => "metadata.load",
            Command::LoadExtendedMetadata { .. } => "metadata.extended",
            Command::Copy { .. } => "ops.copy",
            Command::Move { .. } => "ops.move",
            Command::Delete { .. } => "ops.delete",
            Command::Rename { .. } => "ops.rename",
            Command::CreateFolder { .. } => "ops.create_folder",
            Command::CreateFile { .. } => "ops.create_file",
            Command::Scan { .. } => "scan",
            Command::SetPipeline { .. } => "navigate.pipeline",
            Command::CancelScan { .. } => "scan.cancel",
            Command::CancelOperation { .. } => "ops.cancel",
            Command::Watch { .. } => "watch",
            Command::Unwatch { .. } => "watch.remove",
            Command::UnwatchSession(..) => "watch.session_remove",
            Command::Handshake => "session.handshake",
            Command::DestroySession(..) => "session.destroy",
            Command::Extension { key, .. } => key.as_str(),
        }
    }
}
