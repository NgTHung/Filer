//! Command Router - translates high-level Commands into actor-specific messages
//!
//! The router sits between the API layer and the actor system. It receives
//! `Command` from the FilerCore handle and dispatches them to the appropriate
//! actor channels:
//!
//! - Navigate, NavigateToNode, NavigateUp, Refresh → Navigator (NavCommand)
//! - Search, Cancel (when searching) → Searcher (SearchCommand)
//! - Watch, Unwatch → Watcher (WatchCommand)
//! - LoadPreview, CancelPreview → Previewer (PreviewCommand)
//! - LoadMetadata, LoadExtendedMetadata → Previewer (PreviewCommand)
//! - Copy, Move, Delete, Rename, CreateFolder, CreateFile → Operator (OpsCommand)
//! - Handshake → creates session via SessionManager, emits SessionCreated
//! - DestroySession → tears down session, notifies actors, emits SessionDestroyed
//!
//! ## Session validation
//!
//! Every command (except Handshake) carries a SessionId. The router validates
//! the session exists in SessionManager before routing. Unknown sessions get
//! an `Event::Error` back. This is the single chokepoint for session enforcement.

use flume::{Receiver, Sender};
use tracing::instrument;

use crate::actors::Actor;
use crate::actors::navigator::NavCommand;
use crate::actors::operator::OpsCommand;
use crate::actors::previewer::PreviewCommand;
use crate::actors::scanner::ScanCommand;
use crate::actors::searcher::SearchCommand;
use crate::actors::watcher::WatchCommand;
use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::session_manager::SessionManager;
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;

/// Command Router actor - dispatches Commands to the correct actor channels
#[derive(Debug)]
pub struct CommandRouter {
    /// Incoming high-level commands from the API
    commands: Receiver<Command>,
    /// Outgoing events (session lifecycle, errors)
    events: Sender<Event>,
    /// Channel to Navigator actor
    nav_tx: Sender<NavCommand>,
    /// Channel to Scanner actor
    scan_tx: Sender<ScanCommand>,
    /// Channel to Searcher actor
    search_tx: Sender<SearchCommand>,
    /// Channel to Watcher actor
    watch_tx: Sender<WatchCommand>,
    /// Channel to Previewer actor
    preview_tx: Sender<PreviewCommand>,
    /// Channel to Operator actor
    ops_tx: Sender<OpsCommand>,
    /// Session manager - validates and manages sessions
    session_manager: SessionManager,
    register: NodeRegistry,
}

impl CommandRouter {
    /// Create a new command router with all actor channels
    pub fn new(
        commands: Receiver<Command>,
        events: Sender<Event>,
        nav_tx: Sender<NavCommand>,
        scan_tx: Sender<ScanCommand>,
        search_tx: Sender<SearchCommand>,
        watch_tx: Sender<WatchCommand>,
        preview_tx: Sender<PreviewCommand>,
        ops_tx: Sender<OpsCommand>,
        session_manager: SessionManager,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events,
            nav_tx,
            scan_tx,
            search_tx,
            watch_tx,
            preview_tx,
            ops_tx,
            session_manager,
            register: registry,
        }
    }

    /// Route a single command to the appropriate actor.
    ///
    /// Session validation happens once here — `Command::session_id()` extracts
    /// the session from any variant. If it returns `Some(id)` and the session
    /// is unknown, we emit `Event::Error` and bail. Commands that return `None`
    /// (Handshake, DestroySession, Unwatch) skip validation.
    #[instrument]
    async fn route(&self, command: Command) {
        // ── Single-point session validation ───────────────────────────
        if let Some(session) = command.session_id() {
            if !self.session_manager.exists(session) {
                let _ = self.events.send(Event::Error {
                    message: format!("Unknown session: {}", session),
                    recoverable: true,
                    session,
                });
                return;
            }
        }

        let _ = match command {
            // ── Session lifecycle ─────────────────────────────────────────
            Command::Handshake => {
                let new_session = self.session_manager.create_session(self.events.clone());
                self.events
                    .send_async(Event::SessionCreated(new_session))
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to send SessionCreated: {}", e);
                    })
            }
            Command::DestroySession(session_id) => {
                let _ = self
                    .watch_tx
                    .send_async(WatchCommand::UnwatchSession(session_id))
                    .await;
                let _ = self
                    .nav_tx
                    .send_async(NavCommand::RemoveSession(session_id))
                    .await;
                self.session_manager.remove(session_id);
                self.events
                    .send_async(Event::SessionDestroyed(session_id))
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to send SessionDestroyed: {}", e);
                    })
            }

            // ── Navigation ───────────────────────────────────────────────
            Command::Navigate(path_buf, session_id) => self
                .nav_tx
                .send_async(NavCommand::NavigateToPath {
                    session: session_id,
                    path: path_buf,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::NavigateToNode(node_id, session_id) => self
                .nav_tx
                .send_async(NavCommand::Navigate {
                    session: session_id,
                    node: node_id,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::NavigateUp(session_id) => self
                .nav_tx
                .send_async(NavCommand::Up(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),
            Command::NavigateBack(session_id) => self
                .nav_tx
                .send_async(NavCommand::Back(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),
            Command::Refresh(session_id) => self
                .nav_tx
                .send_async(NavCommand::Refresh(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            // ── Search ───────────────────────────────────────────────────
            Command::Search {
                query,
                root,
                session,
            } => {
                if let Ok(search_query) = SearchQuery::parse(&query) {
                    let _ = self
                        .search_tx
                        .send_async(SearchCommand::Search {
                            query: search_query,
                            root,
                            session,
                        })
                        .await
                        .map_err(|e| tracing::error!("Found error! {}", e));
                    ()
                }
                Err(())
            }

            Command::SearchPath {
                query,
                root,
                session,
            } => {
                if let Ok(search_query) = SearchQuery::parse(&query) {
                    let node = self.register.clone().register(root);
                    let _ = self
                        .search_tx
                        .send_async(SearchCommand::Search {
                            query: search_query,
                            root: node,
                            session,
                        })
                        .await
                        .map_err(|e| tracing::error!("Found error! {}", e));
                    ()
                }
                Err(())
            }

            Command::Cancel(session_id) => self
                .search_tx
                .send_async(SearchCommand::Cancel(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            // ── Preview / Metadata ───────────────────────────────────────
            Command::LoadPreview {
                id,
                options,
                session,
            } => self
                .preview_tx
                .send_async(PreviewCommand::Generate {
                    path: id,
                    options,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::CancelPreview(session_id) => self
                .preview_tx
                .send_async(PreviewCommand::Cancel(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::LoadMetadata(node_id, session_id) => self
                .preview_tx
                .send_async(PreviewCommand::LoadMetadata(node_id, session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::LoadExtendedMetadata(node_id, session_id) => self
                .preview_tx
                .send_async(PreviewCommand::LoadExtendedMetadata(node_id, session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            // ── File operations ──────────────────────────────────────────
            Command::Copy {
                sources,
                destination,
                session,
            } => self
                .ops_tx
                .send_async(OpsCommand::Copy {
                    sources,
                    destination,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::Move {
                sources,
                destination,
                session,
            } => self
                .ops_tx
                .send_async(OpsCommand::Move {
                    sources,
                    destination,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::Delete {
                nodes,
                trash,
                session,
            } => self
                .ops_tx
                .send_async(OpsCommand::Delete {
                    targets: nodes,
                    trash,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::Rename {
                node,
                new_name,
                session,
            } => self
                .ops_tx
                .send_async(OpsCommand::Rename {
                    source: node,
                    new_name,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::CreateFolder {
                parent,
                name,
                session,
            } => self
                .ops_tx
                .send_async(OpsCommand::CreateFolder {
                    parent,
                    name,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::CreateFile {
                parent,
                name,
                session,
            } => self
                .ops_tx
                .send_async(OpsCommand::CreateFile {
                    parent,
                    name,
                    session,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            // ── Scanner ──────────────────────────────────────────────────
            Command::Scan {
                path,
                session,
                pipeline,
            } => self
                .scan_tx
                .send_async(ScanCommand::Scan {
                    path,
                    session,
                    pipeline,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::ScanNode {
                node,
                session,
                pipeline,
            } => self
                .scan_tx
                .send_async(ScanCommand::ScanNode {
                    node,
                    session,
                    pipeline,
                })
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::CancelScan(session_id) => self
                .scan_tx
                .send_async(ScanCommand::Cancel(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            // ── Watch ────────────────────────────────────────────────────
            Command::Watch(node_id, session_id) => self
                .watch_tx
                .send_async(WatchCommand::Watch(node_id, session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::Unwatch(node_id) => self
                .watch_tx
                .send_async(WatchCommand::Unwatch(node_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),

            Command::UnwatchSession(session_id) => self
                .watch_tx
                .send_async(WatchCommand::UnwatchSession(session_id))
                .await
                .map_err(|e| tracing::error!("Found error! {}", e)),
        };
    }
}

impl Actor for CommandRouter {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(command) => {
                    self.route(command).await;
                }
                Err(_) => {
                    // Command channel closed, shut down
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "command-router"
    }
}
