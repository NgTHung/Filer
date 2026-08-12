//! Scan module — directory listing via pluggable `FsProvider`.
//!
//! This module owns the Scanner actor and registers handlers for:
//! - `scan` — scan by Location
//! - `scan.path.compat` — compatibility scan by path
//! - `scan.node.compat` — compatibility scan by NodeId
//! - `scan.cancel` — cancel active scan
//!
//! # Swapping the scanner
//!
//! Supply a different `FsProvider` implementation to change how directory
//! listings work (e.g., virtual FS, remote, cached):
//!
//! ```ignore
//! let scan = ScanModule::new(Arc::new(MyCustomFs::new()));
//! core.load(scan);
//! ```

#[cfg(not(test))]
mod paging;
#[cfg(test)]
pub(crate) mod paging;
#[cfg(test)]
pub(crate) use paging::PageSelection;
pub mod scanner;

use std::sync::Arc;

use flume::Sender;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::model::location::{Location, LocationRef};
use crate::modules::compat;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn;
use crate::vfs::provider::FsProvider;
use scanner::{ScanCommand, Scanner};

/// Scan module — owns the Scanner actor and its command channel.
///
/// Create this before `NavigationModule` and pass `sender()` to it,
/// since the Navigator triggers scans via the scanner channel.
pub struct ScanModule {
    provider: Arc<dyn FsProvider>,
    scan_tx: Sender<ScanCommand>,
    scan_rx: Option<flume::Receiver<ScanCommand>>,
    cache: Option<SharedDirCache>,
}

impl ScanModule {
    pub fn new(provider: Arc<dyn FsProvider>) -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            provider,
            scan_tx: tx,
            scan_rx: Some(rx),
            cache: None,
        }
    }

    pub fn with_cache(provider: Arc<dyn FsProvider>, cache: SharedDirCache) -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            provider,
            scan_tx: tx,
            scan_rx: Some(rx),
            cache: Some(cache),
        }
    }

    /// Get a sender to the scanner channel.
    ///
    /// Call this before `core.load(scan)` to wire into other modules
    /// (e.g., NavigationModule needs this to trigger scans).
    pub fn sender(&self) -> Sender<ScanCommand> {
        self.scan_tx.clone()
    }
}

impl Module for ScanModule {
    fn init(mut self: Box<Self>, ctx: ModuleContext<'_>) {
        let scan_rx = self.scan_rx.take().expect("ScanModule already initialized");

        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan.path.compat", move |cmd, _ctx| {
            if let Command::ScanPathCompat {
                path,
                session,
                pipeline,
                load,
                request,
            } = cmd
            {
                let location = Location::local(path);
                send_or_warn(
                    &tx,
                    ScanCommand::ScanCompat {
                        location: LocationRef::from_location(&location),
                        session,
                        pipeline,
                        load,
                        request,
                    },
                    "scan.path.compat",
                );
            }
        });

        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan", move |cmd, _ctx| {
            if let Command::Scan {
                location,
                session,
                pipeline,
                load,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    ScanCommand::ScanLocation {
                        location,
                        session,
                        pipeline,
                        load,
                        request,
                    },
                    "scan",
                );
            }
        });

        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan.node.compat", move |cmd, ctx| {
            if let Command::ScanNodeCompat {
                node,
                session,
                pipeline,
                load,
                request,
            } = cmd
            {
                let Ok(location) = compat::resolve_node_location(&ctx.registry, node) else {
                    compat::emit_unresolved_node_request(
                        &ctx.events,
                        node,
                        session,
                        request,
                        "scan.node.compat resolve",
                    );
                    return;
                };
                send_or_warn(
                    &tx,
                    ScanCommand::ScanCompat {
                        location,
                        session,
                        pipeline,
                        load,
                        request,
                    },
                    "scan.node.compat",
                );
            }
        });

        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan.cancel", move |cmd, _ctx| {
            if let Command::CancelScan { session } = cmd {
                send_or_warn(&tx, ScanCommand::Cancel(session), "scan.cancel");
            }
        });

        let scanner = match self.cache.take() {
            Some(cache) => Scanner::with_cache(
                scan_rx,
                ctx.events.clone(),
                self.provider.clone(),
                ctx.registry.clone(),
                cache,
            )
            .with_work_tracker(ctx.actors.work_tracker()),
            None => Scanner::new(
                scan_rx,
                ctx.events.clone(),
                self.provider.clone(),
                ctx.registry.clone(),
            )
            .with_work_tracker(ctx.actors.work_tracker()),
        };
        ctx.actors.spawn(scanner);
    }
}
