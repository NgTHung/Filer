//! Search module — file search across directories.
//!
//! This module owns the Searcher actor and registers handlers for:
//! - `search` — search by Location
//! - `search.cancel` — cancel ongoing search

pub mod searcher;

use std::sync::Arc;

use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::module::{Module, ModuleContext};
use crate::errors::CoreError;
use crate::model::query::SearchQuery;
use crate::utils::channel::send_or_warn;
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
                        send_or_warn(
                            &tx,
                            SearchCommand::Search {
                                query,
                                root,
                                event_mode: SearchEventMode::Location,
                                session,
                                request,
                            },
                            "search",
                        );
                    }
                    Err(error) => emit_query_error(ctx, session, request, error),
                }
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on("search.cancel", move |cmd, _ctx| {
            if let Command::CancelSearch { session } = cmd {
                send_or_warn(&tx, SearchCommand::Cancel(session), "search.cancel");
            }
        });

        let tx = search_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            send_or_warn(
                &tx,
                SearchCommand::Cancel(session),
                "search session destroy",
            );
        });

        let searcher = searcher::Searcher::new(
            search_rx,
            ctx.events.clone(),
            self.provider.clone(),
            ctx.registry.clone(),
        )
        .with_work_tracker(ctx.actors.work_tracker());
        ctx.actors.spawn(searcher);
    }
}

fn emit_query_error(
    ctx: &crate::api::module::HandlerContext,
    session: crate::model::session::SessionId,
    request: crate::model::request::RequestId,
    error: crate::model::query::QueryParseError,
) {
    send_or_warn(
        &ctx.events,
        Event::from_request_error(
            CoreError::invalid_input(format!("Invalid search query: {error}")),
            session,
            request,
        ),
        "search query parse",
    );
}
