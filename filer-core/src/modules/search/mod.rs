//! Search module — file search across directories.
//!
//! This module owns the Searcher actor and registers handlers for:
//! - `search` — search by query
//! - `search.cancel` — cancel ongoing search

pub mod searcher;

use std::sync::Arc;

use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::module::{Module, ModuleContext};
use crate::errors::CoreError;
use crate::model::query::SearchQuery;
use crate::vfs::provider::FsProvider;
use searcher::SearchCommand;

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

        // ── Search ───────────────────────────────────────────────────
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
                            session,
                            request,
                        });
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on("search.path", move |cmd, ctx| {
            if let Command::SearchPath {
                query,
                root,
                session,
                request,
            } = cmd
            {
                match SearchQuery::parse(&query) {
                    Ok(query) => {
                        let _ = tx.send(SearchCommand::SearchPath {
                            query,
                            root,
                            session,
                            request,
                        });
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on("search.location", move |cmd, ctx| {
            if let Command::SearchLocation {
                query,
                root,
                session,
                request,
            } = cmd
            {
                match SearchQuery::parse(&query) {
                    Ok(query) => {
                        let _ = tx.send(SearchCommand::SearchLocation {
                            query,
                            root,
                            session,
                            request,
                        });
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        // ── Cancel search ────────────────────────────────────────────
        let tx = search_tx.clone();
        ctx.handlers.on("search.cancel", move |cmd, _ctx| {
            if let Command::Cancel(session) = cmd {
                let _ = tx.send(SearchCommand::Cancel(session));
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
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
