use std::path::PathBuf;
use flume::{Receiver, Sender};

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::query::SearchQuery;
use crate::vfs::provider::FsProvider;

/// Commands for searcher actor
#[derive(Debug, Clone)]
pub enum SearchCommand {
    Search { query: SearchQuery, root: PathBuf },
    Cancel,
}

/// Searcher actor - handles file search
pub struct Searcher {
    commands: Receiver<SearchCommand>,
    events: Sender<Event>,
    provider: Box<dyn FsProvider>,
}

impl Searcher {
    pub fn new(
        commands: Receiver<SearchCommand>,
        events: Sender<Event>,
        provider: Box<dyn FsProvider>,
    ) -> Self {
        todo!()
    }
}

impl Actor for Searcher {
    async fn run(self) {
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "searcher"
    }
}