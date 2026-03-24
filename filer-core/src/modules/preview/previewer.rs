use flume::{Receiver, Sender};
use std::path::PathBuf;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::services::preview::PreviewCache;
use crate::{MetadataRegistry, PreviewOptions, PreviewRegistry};

/// Commands for previewer actor
#[derive(Debug, Clone)]
pub enum PreviewCommand {
    /// Generate preview for a file
    Generate {
        path: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
    },
    /// Load metadata for a file
    LoadMetadata(NodeId, SessionId),
    LoadExtendedMetadata(NodeId, SessionId),
    /// Cancel ongoing preview
    Cancel(SessionId),
    /// Clear cache
    ClearCache,
}

/// Previewer actor - generates file previews
pub struct Previewer {
    commands: Receiver<PreviewCommand>,
    events: Sender<Event>,
    preview_registry: PreviewRegistry,
    metadata_registry: MetadataRegistry,
    cache: PreviewCache,
}

impl Previewer {
    pub fn new(_commands: Receiver<PreviewCommand>, _events: Sender<Event>) -> Self {
        todo!()
    }

    async fn handle_generate(&mut self, _path: PathBuf, _options: Option<PreviewOptions>) {
        todo!()
    }

    async fn handle_metadata(&self, _path: PathBuf) {
        todo!()
    }
}

impl Actor for Previewer {
    async fn run(self) {
        todo!()
    }

    fn name(&self) -> &'static str {
        "previewer"
    }
}
