use flume::{Receiver, Sender};
use std::path::PathBuf;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::services::preview::PreviewCache;
use crate::vfs::provider::FsProvider;
use crate::{MetadataRegistry, PreviewOptions, PreviewRegistry};

/// Commands for previewer actor
#[derive(Debug, Clone)]
pub enum PreviewCommand {
    /// Generate preview for a file
    Generate {
        path: PathBuf,
        options: Option<PreviewOptions>,
    },
    /// Load metadata for a file
    LoadMetadata(PathBuf),
    /// Cancel ongoing preview
    Cancel(PathBuf),
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
    pub fn new(commands: Receiver<PreviewCommand>, events: Sender<Event>) -> Self {
        todo!()
    }

    async fn handle_generate(&mut self, path: PathBuf, options: Option<PreviewOptions>) {
        todo!()
    }

    async fn handle_metadata(&self, path: PathBuf) {
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
