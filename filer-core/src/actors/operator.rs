//! Operations actor - handles file system write operations
//!
//! The Operations actor is responsible for:
//! - Copy (single file, directory recursive, with progress events)
//! - Move (same filesystem, cross filesystem)
//! - Delete (to trash, permanent)
//! - Rename (file, directory)
//! - Create (folder, file)
//!
//! Each operation emits progress events and a final OperationComplete event.
//! Operations are session-scoped and cancellable.

use flume::{Receiver, Sender};

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::session::SessionId;

/// Commands for the operations actor
#[derive(Debug, Clone)]
pub enum OpsCommand {
    /// Copy files/directories to a destination
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
    },
    /// Move files/directories to a destination
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
    },
    /// Delete files/directories
    Delete {
        targets: Vec<NodeId>,
        trash: bool,
        session: SessionId,
    },
    /// Rename a file or directory
    Rename {
        source: NodeId,
        new_name: String,
        session: SessionId,
    },
    /// Create a new folder
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId,
    },
    /// Create a new file
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId,
    },
    /// Cancel an ongoing operation for a session
    Cancel(SessionId),
}

/// Progress info emitted during long-running operations
#[derive(Debug, Clone)]
pub struct OpsProgress {
    /// Total bytes to process
    pub total_bytes: u64,
    /// Bytes processed so far
    pub bytes_done: u64,
    /// Total items to process
    pub total_items: usize,
    /// Items processed so far
    pub items_done: usize,
    /// Current file being processed
    pub current_file: NodeId,
    /// Associated session
    pub session: SessionId,
}

/// Operations actor - executes file system write operations
pub struct Operator {
    /// Incoming operation commands
    commands: Receiver<OpsCommand>,
    /// Outgoing events (progress, completion, errors)
    events: Sender<Event>,
}

impl Operator {
    pub fn new(commands: Receiver<OpsCommand>, events: Sender<Event>) -> Self {
        Self { commands, events }
    }
}

impl Actor for Operator {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(command) => {
                    self.handle(command).await;
                }
                Err(_) => {
                    // Command channel closed, shut down
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}

impl Operator {
    /// Dispatch an operation command
    async fn handle(&self, command: OpsCommand) {
        todo!("Implement operation handling")
    }
}
