use flume::{Receiver, Sender};

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::query::SearchQuery;
use crate::model::session::SessionId;
use crate::vfs::provider::FsProvider;

/// Commands for searcher actor
#[derive(Debug, Clone)]
pub enum SearchCommand {
    Search { query: SearchQuery, root: NodeId, session: SessionId },
    Cancel(SessionId),
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