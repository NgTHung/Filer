use flume::{Receiver, Sender};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
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

/// Scanner actor - handles directory traversal
pub struct Scanner {
    commands: Receiver<ScanCommand>,
    events_sender: Sender<Event>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_scans: CancelMap,
    cache: Option<SharedDirCache>,
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
            active_scans: CancelMap::new(),
            cache: None,
        }
    }

    pub fn with_cache(
        commands: Receiver<ScanCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        Self {
            commands,
            events_sender: events,
            provider,
            registry,
            active_scans: CancelMap::new(),
            cache: Some(cache),
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
        let cache = self.cache.clone();

        let cancel = active_scans.arm(session);
        tokio::spawn(async move {
            Self::scan_directory(
                &provider,
                &registry,
                &events,
                &path,
                session,
                pipeline_config,
                &cancel,
                cache.as_ref(),
            )
            .await;
            active_scans.remove(session).await;
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
        path: &Path,
        session: SessionId,
        pipeline_config: PipelineConfig,
        cancel: &CancellationToken,
        cache: Option<&SharedDirCache>,
    ) {
        // 1. Cache check (before I/O)
        let cached_nodes = cache.and_then(|c| c.lock().ok()?.get(path));
        if let Some(cached) = cached_nodes {
            let parent_id = registry.clone().register(path.to_path_buf());
            registry.clone().register_batch_file_node(&cached);
            let pipeline = Pipeline::from_config(&pipeline_config);
            let groups = pipeline.execute_grouped(cached);
            send_or_warn_async(events, Event::DirectoryLoaded {
                parent: parent_id,
                path: path.to_path_buf(),
                groups,
                session,
            }, "scan result (cached)").await;
            return;
        }

        // 2. List directory (cache miss)
        let entries = match provider.list(path).await {
            Ok(entries) => entries,
            Err(e) => {
                send_or_warn_async(events, Event::from_error(e, session), "scan error").await;
                return;
            }
        };

        // Populate cache after successful list
        if let Some(cache) = cache {
            if let Ok(mut c) = cache.lock() {
                c.put(path.to_path_buf(), entries.clone());
            }
        }

        // 3. Check cancellation after I/O
        if cancel.is_cancelled() {
            return;
        }

        // 4. Register nodes
        let parent_id = registry.clone().register(path.to_path_buf());
        registry.clone().register_batch_file_node(&entries);

        // 5. Execute pipeline (always returns GroupedNodes)
        let pipeline = Pipeline::from_config(&pipeline_config);
        let groups = pipeline.execute_grouped(entries);

        // 5. Check cancellation after pipeline
        if cancel.is_cancelled() {
            return;
        }

        // 6. Send result
        send_or_warn_async(events, Event::DirectoryLoaded {
            parent: parent_id,
            path: path.to_path_buf(),
            groups,
            session,
        }, "scan result").await;
    }

    fn cancel_scan(&self, session: SessionId) {
        self.active_scans.cancel(session);
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
                        send_or_warn(&self.events_sender, Event::Error {
                            message: format!("Unable to resolve ID: {node:?}"),
                            recoverable: false,
                            session,
                        }, "scan resolve error");
                        continue;
                    };
                    self.dispatch_scan(path, session, pipeline);
                }
                Ok(ScanCommand::Cancel(session)) => {
                    self.cancel_scan(session);
                }
                Err(_) | Ok(ScanCommand::Shutdown) => {
                    self.active_scans.cancel_all().await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "scanner"
    }
}
