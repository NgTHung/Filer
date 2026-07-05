//! Search module — file search across directories.
//!
//! This module owns the Searcher actor and registers handlers for:
//! - `search` — search by Location
//! - `search.node.compat` — compatibility search by NodeId
//! - `search.path.compat` — compatibility search by path
//! - `search.cancel` — cancel ongoing search

pub mod searcher;

use std::sync::Arc;

use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::module::{Module, ModuleContext};
use crate::errors::CoreError;
use crate::model::location::{Location, LocationRef};
use crate::model::query::SearchQuery;
use crate::vfs::provider::FsProvider;
use searcher::{SearchCommand, SearchEventMode};

/// Search module — owns the Searcher actor.
pub struct SearchModule {
    provider: Arc<dyn FsProvider>,
}

impl SearchModule {
    pub fn new(provider: Arc<dyn FsProvider>) -> Self {
        Self { provider }
    }
}

impl Module for SearchModule {
    fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
        let (search_tx, search_rx) = flume::unbounded::<SearchCommand>();

        let tx = search_tx.clone();
        ctx.handlers.on("search.node.compat", move |cmd, ctx| {
            if let Command::SearchNodeCompat {
                query,
                root: node_root,
                session,
                request,
            } = cmd
            {
                match SearchQuery::parse(&query) {
                    Ok(query) => {
                        let Some(root) = ctx.registry.resolve_node_location(node_root) else {
                            let _ = ctx.events.send(Event::from_request_error(
                                CoreError::invalid_input(format!(
                                    "Unable to resolve ID: {node_root:?}"
                                )),
                                session,
                                request,
                            ));
                            return;
                        };
                        let _ = tx.send(SearchCommand::Search {
                            query,
                            root,
                            event_mode: SearchEventMode::Compat,
                            session,
                            request,
                        });
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on("search.path.compat", move |cmd, ctx| {
            if let Command::SearchPathCompat {
                query,
                root,
                session,
                request,
            } = cmd
            {
                match SearchQuery::parse(&query) {
                    Ok(query) => {
                        let location = Location::local(root);
                        let _ = tx.send(SearchCommand::Search {
                            query,
                            root: LocationRef::from_location(&location),
                            event_mode: SearchEventMode::Compat,
                            session,
                            request,
                        });
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on("search", move |cmd, ctx| {
            if let Command::Search {
                query,
                root,
                session,
                request,
            } = cmd
            {
                match SearchQuery::parse(&query) {
                    Ok(query) => {
                        let _ = tx.send(SearchCommand::Search {
                            query,
                            root,
                            event_mode: SearchEventMode::Location,
                            session,
                            request,
                        });
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on("search.cancel", move |cmd, _ctx| {
            if let Command::CancelSearch { session } = cmd {
                let _ = tx.send(SearchCommand::Cancel(session));
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(SearchCommand::Cancel(session));
        });

        let searcher = searcher::Searcher::new(
            search_rx,
            ctx.events.clone(),
            self.provider.clone(),
            ctx.registry.clone(),
        );
        ctx.actors.spawn(searcher);
    }
}

fn emit_query_error(
    ctx: &crate::api::module::HandlerContext,
    session: crate::model::session::SessionId,
    request: crate::model::request::RequestId,
    error: crate::model::query::QueryParseError,
) {
    let _ = ctx.events.send(Event::from_request_error(
        CoreError::invalid_input(format!("Invalid search query: {error}")),
        session,
        request,
    ));
}
