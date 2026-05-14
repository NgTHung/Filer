use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::Event;
use crate::errors::ErrorKind;
use crate::model::node::NodeId;
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
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
        request: RequestId,
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
    latest_searches: Arc<scc::HashMap<SessionId, RequestId, RandomState>>,
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
            latest_searches: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }
    pub fn dispatch_search(
        &self,
        query: SearchQuery,
        path: PathBuf,
        session: SessionId,
        request: RequestId,
    ) {
        let provider = self.provider.clone();
        let active_search = self.active_search.clone();
        let event = self.events.clone();
        let latest_searches = self.latest_searches.clone();
        let _ = self.latest_searches.remove_sync(&session);
        let _ = self.latest_searches.insert_sync(session, request);
        let cancel = self.active_search.arm(session);

        tokio::spawn(async move {
            Self::search(
                query,
                path,
                session,
                request,
                &provider,
                &cancel,
                &event,
                &latest_searches,
            )
            .await;
            active_search.remove(session).await;
        });
    }

    async fn search(
        query: SearchQuery,
        path: PathBuf,
        session: SessionId,
        request: RequestId,
        provider: &Arc<dyn FsProvider>,
        cancel: &CancellationToken,
        event: &Sender<Event>,
        latest_searches: &scc::HashMap<SessionId, RequestId, RandomState>,
    ) {
        let mut queue = VecDeque::new();
        let mut batch = vec![];
        let mut total_found = 0;
        queue.push_back((path, 0));
        'outer: while let Some((dir, depth)) = queue.pop_front() {
            if cancel.is_cancelled() {
                break;
            }
            let Ok(entries) = provider.list(&dir).await else {
                continue;
            };
            if cancel.is_cancelled() {
                break;
            }
            for entry in entries {
                if !query.options.include_hidden && entry.meta.hidden {
                    continue;
                }
                if entry.is_dir() && query.options.max_depth.is_none_or(|v| depth < v) {
                    queue.push_back((entry.path.clone(), depth + 1));
                }
                if query.matches(&entry) {
                    batch.push(entry);
                    total_found += 1;
                    if batch.len() >= query.options.batch_size.unwrap_or(DEFAULT_BATCH_SIZE) {
                        if !Self::is_latest(latest_searches, session, request) {
                            return;
                        }
                        send_or_warn(
                            event,
                            Event::SearchResults {
                                matches: std::mem::take(&mut batch),
                                complete: false,
                                session,
                                request,
                            },
                            "emit partial search result",
                        );
                    }
                }
                if query
                    .options
                    .max_results
                    .is_some_and(|max| total_found >= max)
                {
                    break 'outer;
                }
            }
        }
        if Self::is_latest(latest_searches, session, request) {
            send_or_warn(
                event,
                Event::SearchResults {
                    matches: batch,
                    complete: true,
                    session,
                    request,
                },
                "emit remaining files after search",
            );
        }
    }

    fn is_latest(
        latest_searches: &scc::HashMap<SessionId, RequestId, RandomState>,
        session: SessionId,
        request: RequestId,
    ) -> bool {
        latest_searches
            .read_sync(&session, |_, latest| *latest == request)
            .unwrap_or(false)
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
                    request,
                }) => {
                    let Some(path) = self.registry.resolve(root) else {
                        send_or_warn(
                            &self.events,
                            Event::Error {
                                kind: ErrorKind::InvalidInput,
                                message: format!("Unable to resolve ID: {root:?}"),
                                recoverable: true,
                                session,
                                request: Some(request),
                                operation: None,
                            },
                            "search resolve error",
                        );
                        continue;
                    };
                    self.dispatch_search(query, path, session, request);
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
