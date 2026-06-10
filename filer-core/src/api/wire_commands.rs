//! # Wire commands
//!
//! This module defines the serializable DTO for built-in commands. It mirrors
//! the runtime command surface while leaving type-erased extension payloads in
//! process until a wire-safe extension contract exists.
//!
//! ```
//! use filer_core::model::session::SessionId;
//! use filer_core::{Command, Location, LocationRef, RequestId, WireCommand};
//!
//! let location = Location::local("/tmp");
//! let wire = WireCommand::Navigate {
//!     location: LocationRef::from_location(&location),
//!     session: SessionId::DEFAULT,
//!     request: RequestId::new(),
//! };
//! let json = serde_json::to_string(&wire)?;
//! let runtime = Command::from(serde_json::from_str::<WireCommand>(&json)?);
//! assert_eq!(runtime.key(), "navigate");
//! # Ok::<(), serde_json::Error>(())
//! ```

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::PreviewOptions;
use crate::api::commands::Command;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::LocationRef;
use crate::model::node::NodeId;
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;

macro_rules! built_in_wire_commands {
    (
        $(
            $variant:ident {
                $( $field:ident : $ty:ty ),* $(,)?
            }
        ),* $(,)?
    ) => {
        /// Serializable built-in command DTO.
        ///
        /// The `type` field is an internally tagged snake_case variant name.
        /// This DTO is unversioned; transport envelopes belong to the protocol
        /// layer.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub enum WireCommand {
            $(
                $variant {
                    $( $field: $ty ),*
                },
            )*
            UnwatchSession {
                session: SessionId,
            },
            Handshake,
            DestroySession {
                session: SessionId,
            },
        }

        impl From<WireCommand> for Command {
            fn from(command: WireCommand) -> Self {
                match command {
                    $(
                        WireCommand::$variant { $( $field ),* } => {
                            Command::$variant { $( $field ),* }
                        }
                    )*
                    WireCommand::UnwatchSession { session } => Command::UnwatchSession(session),
                    WireCommand::Handshake => Command::Handshake,
                    WireCommand::DestroySession { session } => Command::DestroySession(session),
                }
            }
        }

        impl TryFrom<Command> for WireCommand {
            type Error = WireCommandConversionError;

            fn try_from(command: Command) -> Result<Self, Self::Error> {
                match command {
                    $(
                        Command::$variant { $( $field ),* } => {
                            Ok(WireCommand::$variant { $( $field ),* })
                        }
                    )*
                    Command::UnwatchSession(session) => {
                        Ok(WireCommand::UnwatchSession { session })
                    }
                    Command::Handshake => Ok(WireCommand::Handshake),
                    Command::DestroySession(session) => {
                        Ok(WireCommand::DestroySession { session })
                    }
                    Command::Extension { key, .. } => {
                        Err(WireCommandConversionError::ExtensionUnsupported { key })
                    }
                }
            }
        }
    };
}

built_in_wire_commands! {
    NavigatePathCompat {
        path: PathBuf,
        session: SessionId,
        request: RequestId,
    },
    Navigate {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },
    NavigateNodeCompat {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },
    NavigateUp {
        session: SessionId,
        request: RequestId,
    },
    NavigateBack {
        session: SessionId,
        request: RequestId,
    },
    NavigateForward {
        session: SessionId,
        request: RequestId,
    },
    Refresh {
        session: SessionId,
        request: RequestId,
    },
    SearchNodeCompat {
        query: String,
        root: NodeId,
        session: SessionId,
        request: RequestId,
    },
    SearchPathCompat {
        query: String,
        root: PathBuf,
        session: SessionId,
        request: RequestId,
    },
    Search {
        query: String,
        root: LocationRef,
        session: SessionId,
        request: RequestId,
    },
    CancelSearch {
        session: SessionId,
    },
    LoadPreviewNodeCompat {
        id: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },
    LoadPreview {
        location: LocationRef,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    },
    CancelPreview {
        session: SessionId,
    },
    CopyNodeCompat {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Copy {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    MoveNodeCompat {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Move {
        sources: Vec<LocationRef>,
        destination: LocationRef,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    DeleteNodeCompat {
        nodes: Vec<NodeId>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Delete {
        locations: Vec<LocationRef>,
        trash: bool,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    RenameNodeCompat {
        node: NodeId,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    Rename {
        location: LocationRef,
        new_name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFolderNodeCompat {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFolder {
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFileNodeCompat {
        parent: NodeId,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    CreateFile {
        parent: LocationRef,
        name: String,
        session: SessionId,
        request: RequestId,
        operation: OperationId,
    },
    LoadMetadataNodeCompat {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },
    LoadMetadata {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },
    LoadExtendedMetadataNodeCompat {
        node: NodeId,
        session: SessionId,
        request: RequestId,
    },
    LoadExtendedMetadata {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },
    ScanPathCompat {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    Scan {
        location: LocationRef,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    ScanNodeCompat {
        node: NodeId,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    SetPipeline {
        session: SessionId,
        config: PipelineConfig,
    },
    CancelScan {
        session: SessionId,
    },
    CancelOperation {
        session: SessionId,
        operation: OperationId,
    },
    WatchNodeCompat {
        node: NodeId,
        session: SessionId,
    },
    Watch {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },
    UnwatchNodeCompat {
        node: NodeId,
    },
    Unwatch {
        location: LocationRef,
        session: SessionId,
    },
}

/// Failure converting a runtime command into its built-in wire DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireCommandConversionError {
    /// Runtime extension payloads are type-erased and cannot be serialized.
    ExtensionUnsupported { key: String },
}

impl std::fmt::Display for WireCommandConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExtensionUnsupported { key } => {
                write!(
                    formatter,
                    "extension command '{key}' has no wire-safe payload"
                )
            }
        }
    }
}

impl std::error::Error for WireCommandConversionError {}
