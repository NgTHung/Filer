use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::Receiver;
use rapidhash::fast::RandomState;

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::errors::ErrorCode;
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::node::NodeEntry;
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

const DEFAULT_BATCH_SIZE: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEventMode {
    Location,
}

/// Commands for searcher actor
#[derive(Debug, Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "Keep the public command payload inline to avoid an allocation for each search request."
)]
pub enum SearchCommand {
    Search {
        query: SearchQuery,
        root: LocationRef,
        event_mode: SearchEventMode,
        session: SessionId,
        request: RequestId,
    },
    Cancel(SessionId),
    Shutdown,
}

/// Searcher actor - handles recursive file search
pub struct Searcher {
    commands: Receiver<SearchCommand>,
    events: EventSink,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_search: CancelMap,
    latest_searches: Arc<scc::HashMap<SessionId, RequestId, RandomState>>,
    search_timeout: Option<Duration>,
    work: WorkTracker,
}

impl Searcher {
    pub fn new<E: Into<EventSink>>(
        commands: Receiver<SearchCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events: events.into(),
            provider,
            registry,
            active_search: CancelMap::new(),
            latest_searches: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            search_timeout: None,
            work: WorkTracker::new(),
        }
    }

    pub(crate) fn with_work_tracker(mut self, work: WorkTracker) -> Self {
        self.work = work;
        self
    }

    /// Bound each provider listing during a walk to `timeout`.
    ///
    /// `None` leaves listings unbounded. A breached deadline ends the walk with
    /// a `TimedOut` error instead of skipping the directory.
    pub fn set_search_timeout(&mut self, timeout: Option<Duration>) {
        self.search_timeout = timeout;
    }
    pub fn dispatch_search(
        &self,
        query: SearchQuery,
        root: LocationRef,
        path: PathBuf,
        event_mode: SearchEventMode,
        session: SessionId,
        request: RequestId,
    ) {
        let provider = self.provider.clone();
        let active_search = self.active_search.clone();
        let event = self.events.clone();
        let latest_searches = self.latest_searches.clone();
        let registry = self.registry.clone();
        let work = self.work.clone();
        let _ = self.latest_searches.remove_sync(&session);
        let _ = self.latest_searches.insert_sync(session, request);
        let cancel = self.active_search.arm(session);
        let deadline = self.search_timeout.map(|timeout| Instant::now() + timeout);

        work.spawn(cancel.clone(), async move {
            Self::search(
                query,
                root,
                path,
                session,
                request,
                &provider,
                &cancel,
                deadline,
                &event,
                &latest_searches,
                &registry,
                event_mode,
            )
            .await;
            active_search.remove_if_current(session, &cancel).await;
        });
    }

    #[allow(clippy::too_many_arguments)]
    async fn search(
        query: SearchQuery,
        _root: LocationRef,
        path: PathBuf,
        session: SessionId,
        request: RequestId,
        provider: &Arc<dyn FsProvider>,
        cancel: &CancellationToken,
        deadline: Option<Instant>,
        event: &EventSink,
        latest_searches: &scc::HashMap<SessionId, RequestId, RandomState>,
        registry: &NodeRegistry,
        event_mode: SearchEventMode,
    ) {
        let mut cx = ProviderCx::with_cancel(cancel);
        if let Some(deadline) = deadline {
            cx = cx.with_deadline(deadline);
        }
        let mut queue = VecDeque::new();
        let mut batch: Vec<NodeEntry> = vec![];
        let mut total_found = 0;
        queue.push_back((path, 0));
        'outer: while let Some((dir, depth)) = queue.pop_front() {
            if cancel.is_cancelled() {
                return;
            }
            let entries = match cx.race(provider.scheme(), provider.list(&dir, &cx)).await {
                Ok(entries) => entries,
                Err(e) if e.code() == ErrorCode::Cancelled => return,
                Err(e) if e.code() == ErrorCode::TimedOut => {
                    if Self::is_latest(latest_searches, session, request) {
                        send_or_warn_async(
                            event,
                            Event::from_request_error(e, session, request),
                            "search timed out",
                        )
                        .await;
                    }
                    break;
                }
                Err(_) => continue,
            };
            for entry in entries {
                if !query.options.include_hidden && entry.meta.hidden {
                    continue;
                }
                if entry.is_dir()
                    && query.options.max_depth.is_none_or(|v| depth < v)
                    && let Some(path) = entry.location.descriptor().and_then(|descriptor| {
                        descriptor.route().as_direct_path().map(PathBuf::from)
                    })
                {
                    queue.push_back((path, depth + 1));
                }
                if query.matches(&entry) {
                    batch.push(entry);
                    total_found += 1;
                    if batch.len() >= query.options.batch_size.unwrap_or(DEFAULT_BATCH_SIZE) {
                        if !Self::is_latest(latest_searches, session, request) {
                            return;
                        }
                        send_or_warn_async(
                            event,
                            search_results_event(
                                std::mem::take(&mut batch),
                                false,
                                session,
                                request,
                                registry,
                                event_mode,
                            ),
                            "emit partial search result",
                        )
                        .await;
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
            send_or_warn_async(
                event,
                search_results_event(batch, true, session, request, registry, event_mode),
                "emit remaining files after search",
            )
            .await;
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
                    event_mode,
                    session,
                    request,
                }) => {
                    let location = match self.registry.resolve_location_ref(&root) {
                        Ok(location) => location,
                        Err(error) => {
                            send_or_warn(
                                &self.events,
                                Event::from_request_error(error, session, request),
                                "search resolve error",
                            );
                            continue;
                        }
                    };
                    let route = location.route();
                    let path = match &route {
                        LocationRoute::DirectPath { path } => path.clone(),
                        LocationRoute::Segmented { .. }
                        | LocationRoute::UnsupportedProvider { .. } => {
                            let error = route.require_direct_path().unwrap_err();
                            send_or_warn(
                                &self.events,
                                Event::from_request_error(error, session, request),
                                "search route error",
                            );
                            continue;
                        }
                    };
                    self.dispatch_search(query, root, path, event_mode, session, request);
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

fn search_results_event(
    matches: Vec<NodeEntry>,
    complete: bool,
    session: SessionId,
    request: RequestId,
    _registry: &NodeRegistry,
    _event_mode: SearchEventMode,
) -> Event {
    Event::SearchResults {
        matches,
        complete,
        session,
        request,
    }
}
