use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use flume::{Receiver, Sender};

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::utils::channel::send_or_warn;
use crate::vfs::provider::FsProvider;

const DEFAULT_BATCH_SIZE: usize = 50;

/// Commands for searcher actor
#[derive(Debug, Clone)]
pub enum SearchCommand {
    Search {
        query: SearchQuery,
        root: NodeId,
        session: SessionId,
    },
    Cancel(SessionId),
    Shutdown,
}

/// Searcher actor - handles recursive file search
pub struct Searcher {
    commands: Receiver<SearchCommand>,
    events: Sender<Event>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_search: CancelMap,
}

impl Searcher {
    pub fn new(
        commands: Receiver<SearchCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events,
            provider,
            registry,
            active_search: CancelMap::new(),
        }
    }
    pub fn dispatch_search(&self, query: SearchQuery, path: PathBuf, session: SessionId) {
        let provider = self.provider.clone();
        let active_search = self.active_search.clone();
        let event = self.events.clone();
        let cancel = self.active_search.arm(session);

        tokio::spawn(async move {
            Self::search(query, path, session, &provider, &cancel, &event).await;
            active_search.remove(session).await;
        });
    }

    async fn search(
        query: SearchQuery,
        path: PathBuf,
        session: SessionId,
        provider: &Arc<dyn FsProvider>,
        cancel: &CancellationToken,
        event: &Sender<Event>,
    ) {
        let mut queue = VecDeque::new();
        let mut batch = vec![];
        let mut total_found = 0;
        queue.push_back((path,0));
        'outer: while let Some((dir, depth)) = queue.pop_front() {
            if cancel.is_cancelled(){
                break;
            }
            let Ok(entries) = provider.list(&dir).await else{
                continue;
            };
            if cancel.is_cancelled(){
                break;
            }
            for entry in entries {
                if !query.options.include_hidden && entry.meta.hidden {
                    continue;
                }
                if entry.is_dir()
                    && query.options.max_depth.is_none_or(|v| depth < v){
                        queue.push_back((entry.path.clone(), depth + 1));
                    }
                if query.matches(&entry) {
                    batch.push(entry);
                    total_found += 1;
                    if batch.len() >= query.options.batch_size.unwrap_or(DEFAULT_BATCH_SIZE){
                        send_or_warn(event, Event::SearchResults { matches: std::mem::take(&mut batch), complete: false, session }, "emit partial search result");
                    }
                }
                if query.options.max_results.is_some_and(|max| total_found >= max){
                    break 'outer;
                }
            }
        }
        send_or_warn(event, Event::SearchResults{matches: batch, complete: true, session}, "emit remaining files after search");
    }

    fn cancel_search(&self, session: SessionId) {
        self.active_search.cancel(session);
    }
}

impl Actor for Searcher {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(SearchCommand::Cancel(s)) => {
                    self.cancel_search(s);
                }
                Ok(SearchCommand::Search {
                    query,
                    root,
                    session,
                }) => {
                    let Some(path) = self.registry.resolve(root) else {
                        send_or_warn(&self.events, Event::Error {
                            message: format!("Unable to resolve ID: {root:?}"),
                            recoverable: true,
                            session,
                        }, "search resolve error");
                        continue;
                    };
                    self.dispatch_search(query, path, session);
                }
                Err(_) | Ok(SearchCommand::Shutdown) => {
                    self.active_search.cancel_all().await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "searcher"
    }
}
