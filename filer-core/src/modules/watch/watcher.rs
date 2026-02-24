use flume::Sender;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::session::SessionId;

/// Commands for watcher actor
#[derive(Debug, Clone)]
pub enum WatchCommand {
    Watch(NodeId, SessionId),
    Unwatch(NodeId),
    UnwatchSession(SessionId),
    UnwatchAll,
}

/// Watcher actor - monitors filesystem changes
pub struct Watcher {
    commands: flume::Receiver<WatchCommand>,
    events: Sender<Event>,
}

impl Watcher {
    pub fn new(commands: flume::Receiver<WatchCommand>, events: Sender<Event>) -> Self {
        todo!()
    }
}

impl Actor for Watcher {
    async fn run(self) {
        todo!()
    }

    fn name(&self) -> &'static str {
        "watcher"
    }
}
