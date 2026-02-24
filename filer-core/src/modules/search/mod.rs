//! Search module — file search across directories.
//!
//! This module owns the Searcher actor and registers handlers for:
//! - `search` — search by query
//! - `search.cancel` — cancel ongoing search

pub mod searcher;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::model::query::SearchQuery;
use searcher::SearchCommand;

/// Search module — owns the Searcher actor.
pub struct SearchModule;

impl SearchModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for SearchModule {
    fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
        let (search_tx, _search_rx) = flume::unbounded::<SearchCommand>();

        // ── Search ───────────────────────────────────────────────────
        let tx = search_tx.clone();
        ctx.handlers.on("search", move |cmd, _ctx| {
            if let Command::Search { query, root, session } = cmd {
                if let Ok(query) = SearchQuery::parse(&query){
                    let _ = tx.send(SearchCommand::Search { query, root, session });
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

        // TODO: Spawn Searcher actor once constructor is implemented
        // let searcher = Searcher::new(search_rx, ctx.events.clone(), provider);
        // ctx.actors.spawn(searcher);
    }
}
