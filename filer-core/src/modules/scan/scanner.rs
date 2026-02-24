use flume::{Receiver, Sender};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig};
use crate::vfs::provider::FsProvider;

/// Commands for scanner actor
#[derive(Debug, Clone)]
pub enum ScanCommand {
    Scan {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
    },
    ScanNode {
        node: NodeId,
        session: SessionId,
        pipeline: PipelineConfig,
    },
    Cancel(SessionId),
    Shutdown,
}

#[derive(Clone)]
struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}


impl CancellationToken {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Scanner actor - handles directory traversal
pub struct Scanner {
    commands: Receiver<ScanCommand>,
    events_sender: Sender<Event>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_scans: Arc<scc::HashMap<SessionId, CancellationToken>>,
}

impl Scanner {
    pub fn new(
        commands: Receiver<ScanCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events_sender: events,
            provider,
            registry,
            active_scans: Arc::new(scc::HashMap::new()),
        }
    }

    /// Resolve a scan command to a concrete path, then spawn the scan task.
    ///
    /// Both `Scan` (raw path) and `ScanNode` (NodeId) funnel through here
    /// after resolving to a `PathBuf`.  NodeId resolution is cheap
    /// (in-memory registry lookup) and happens *before* the spawn so we
    /// fail fast without creating cancellation tokens for invalid nodes.
    fn dispatch_scan(&self, path: PathBuf, session: SessionId, pipeline_config: PipelineConfig) {
        let provider = self.provider.clone();
        let registry = self.registry.clone();
        let events = self.events_sender.clone();
        let active_scans = self.active_scans.clone();

        tokio::spawn(async move {
            // Create and register cancellation token, cancelling any
            // existing scan for this session first.
            let cancel = CancellationToken::new();
            if let Some((_, old)) = active_scans.remove_async(&session).await {
                old.cancel();
            }
            let _ = active_scans.insert_async(session, cancel.clone()).await;

            Self::scan_directory(
                &provider,
                &registry,
                &events,
                &path,
                session,
                pipeline_config,
                &cancel,
            )
            .await;

            // Clean up — remove our token so we don't leak entries
            let _ = active_scans.remove_async(&session).await;
        });
    }

    /// Perform a single directory scan: list → register → pipeline → emit.
    ///
    /// Checks the cancellation token at two points:
    /// 1. After the (potentially slow) filesystem listing
    /// 2. After pipeline execution, before sending the result
    async fn scan_directory(
        provider: &Arc<dyn FsProvider>,
        registry: &NodeRegistry,
        events: &Sender<Event>,
        path: &PathBuf,
        session: SessionId,
        pipeline_config: PipelineConfig,
        cancel: &CancellationToken,
    ) {
        // 1. List directory
        let entries = match provider.list(path).await {
            Ok(entries) => entries,
            Err(e) => {
                let _ = events
                    .send_async(Event::Error {
                        message: format!("Failed to scan {}: {}", path.display(), e),
                        recoverable: true,
                        session,
                    })
                    .await;
                return;
            }
        };

        // 2. Check cancellation after I/O
        if cancel.is_cancelled() {
            return;
        }

        // 3. Register nodes
        let parent_id = registry.clone().register(path.clone());
        registry.clone().register_batch_file_node(&entries);

        // 4. Execute pipeline (always returns GroupedNodes)
        let pipeline = Pipeline::from_config(&pipeline_config);
        let groups = pipeline.execute_grouped(entries);

        // 5. Check cancellation after pipeline
        if cancel.is_cancelled() {
            return;
        }

        // 6. Send result
        let _ = events
            .send_async(Event::DirectoryLoaded {
                parent: parent_id,
                path: path.to_path_buf(),
                groups,
                session,
            })
            .await;
    }

    async fn cancel_scan(&self, session: SessionId) {
        if let Some((_, token)) = self.active_scans.remove_async(&session).await {
            token.cancel();
        }
    }
}

impl Actor for Scanner {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(ScanCommand::Scan {
                    path,
                    session,
                    pipeline,
                }) => {
                    self.dispatch_scan(path, session, pipeline);
                }
                Ok(ScanCommand::ScanNode {
                    node,
                    session,
                    pipeline,
                }) => {
                    // Resolve NodeId → PathBuf before spawning.
                    // Cheap in-memory lookup; fail fast if invalid.
                    let Some(path) = self.registry.resolve(node) else {
                        let _ = self.events_sender.send(Event::Error {
                            message: format!("Unable to resolve ID: {node:?}"),
                            recoverable: false,
                            session,
                        });
                        continue;
                    };
                    self.dispatch_scan(path, session, pipeline);
                }
                Ok(ScanCommand::Cancel(session)) => {
                    self.cancel_scan(session).await;
                }
                Err(_) | Ok(ScanCommand::Shutdown) => {
                    self.active_scans
                        .iter_async(|_k, v| {
                            v.cancel();
                            true
                        })
                        .await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "scanner"
    }
}
