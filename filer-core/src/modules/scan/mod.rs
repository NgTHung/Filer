//! Scan module — directory listing via pluggable `FsProvider`.
//!
//! This module owns the Scanner actor and registers handlers for:
//! - `scan` — scan by path
//! - `scan.node` — scan by NodeId
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

pub mod scanner;

use std::sync::Arc;

use flume::Sender;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use scanner::{ScanCommand, Scanner};
use crate::vfs::provider::FsProvider;

/// Scan module — owns the Scanner actor and its command channel.
///
/// Create this before `NavigationModule` and pass `sender()` to it,
/// since the Navigator triggers scans via the scanner channel.
pub struct ScanModule {
    provider: Arc<dyn FsProvider>,
    scan_tx: Sender<ScanCommand>,
    scan_rx: Option<flume::Receiver<ScanCommand>>,
}

impl ScanModule {
    pub fn new(provider: Arc<dyn FsProvider>) -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            provider,
            scan_tx: tx,
            scan_rx: Some(rx),
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

        // ── Register scan command handlers ───────────────────────────
        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan", move |cmd, _ctx| {
            if let Command::Scan {
                path,
                session,
                pipeline,
            } = cmd
            {
                let _ = tx.send(ScanCommand::Scan {
                    path,
                    session,
                    pipeline,
                });
            }
        });

        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan.node", move |cmd, _ctx| {
            if let Command::ScanNode {
                node,
                session,
                pipeline,
            } = cmd
            {
                let _ = tx.send(ScanCommand::ScanNode {
                    node,
                    session,
                    pipeline,
                });
            }
        });

        let tx = self.scan_tx.clone();
        ctx.handlers.on("scan.cancel", move |cmd, _ctx| {
            if let Command::CancelScan(session) = cmd {
                let _ = tx.send(ScanCommand::Cancel(session));
            }
        });

        // ── Spawn Scanner actor ──────────────────────────────────────
        let scanner = Scanner::new(
            scan_rx,
            ctx.events.clone(),
            self.provider.clone(),
            ctx.registry.clone(),
        );
        ctx.actors.spawn(scanner);
    }
}
