use flume::{Receiver, Sender};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig, PipelineData};
use crate::vfs::provider::FsProvider;

/// Commands for scanner actor
#[derive(Debug, Clone)]
pub enum ScanCommand {
    Scan { path: PathBuf, session: SessionId, pipeline: PipelineConfig},
    ScanNode {node: NodeId, session: SessionId, pipeline: PipelineConfig},
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
    provider: Arc<dyn FsProvider>,  // Changed to Arc for sharing
    registry: NodeRegistry,
    active_scans: Arc<scc::HashMap<SessionId, CancellationToken>>,
}

impl Scanner {
    pub fn new(
        commands: Receiver<ScanCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,  // Changed to Arc
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

    /// Spawn a scan task that runs concurrently
    fn spawn_scan(
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        events_sender: Sender<Event>,
        active_scans: Arc<scc::HashMap<SessionId, CancellationToken>>,
        path: PathBuf,
        session: SessionId,
        pipeline_config: PipelineConfig,
    ) {
        tokio::spawn(async move {
            // Create and register cancellation token
            let cancel = CancellationToken::new();
            
            // Cancel any existing scan for this session
            if let Some((_, old)) = active_scans.remove_async(&session).await {
                old.cancel();
            }
            let _ = active_scans.insert_async(session, cancel.clone()).await;

            // Perform the scan
            Self::scan_directory_inner(
                &provider,
                &registry,
                &events_sender,
                &path,
                session,
                pipeline_config,
                &cancel,
            ).await;

            // Clean up
            let _ = active_scans.remove_async(&session).await;
        });
    }

    /// Inner scan logic (static, doesn't need &self)
    async fn scan_directory_inner(
        provider: &Arc<dyn FsProvider>,
        registry: &NodeRegistry,
        events_sender: &Sender<Event>,
        path: &PathBuf,
        session: SessionId,
        pipeline_config: PipelineConfig,
        cancel: &CancellationToken,
    ) {
        // 1. List directory
        let entries = match provider.list(path).await {
            Ok(entries) => entries,
            Err(e) => {
                let _ = events_sender
                    .send_async(Event::Error {
                        message: format!("Failed to scan {}: {}", path.display(), e),
                        recoverable: true,
                        session,
                    })
                    .await;
                return;
            }
        };

        // 2. Check cancellation
        if cancel.is_cancelled() {
            return;
        }

        // 3. Register nodes
        let parent_id = registry.clone().register(path.clone());
        registry.clone().register_batch_file_node(&entries);

        // 4. Execute pipeline
        let pipeline = Pipeline::from_config(&pipeline_config);
        let processed = pipeline.execute(entries);

        let final_entries = match processed {
            PipelineData::Flat(file_nodes) => file_nodes,
            PipelineData::Grouped(grouped_nodes) => grouped_nodes
                .groups
                .into_iter()
                .flat_map(|g| g.nodes)
                .collect(),
        };

        // 5. Check cancellation again
        if cancel.is_cancelled() {
            return;
        }

        // 6. Send result
        let _ = events_sender
            .send_async(Event::DirectoryLoaded {
                parent: parent_id,
                path: path.to_path_buf(),
                entries: final_entries,
                session,
            })
            .await;
    }

    fn spawn_scan_node(
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        events_sender: Sender<Event>,
        active_scans: Arc<scc::HashMap<SessionId, CancellationToken>>,
        node: NodeId,
        session: SessionId,
        pipeline_config: PipelineConfig,
    ) {
        tokio::spawn(async move {
            // Create and register cancellation token
            let cancel = CancellationToken::new();
            
            // Cancel any existing scan for this session
            if let Some((_, old)) = active_scans.remove_async(&session).await {
                old.cancel();
            }
            let _ = active_scans.insert_async(session, cancel.clone()).await;

            // Perform the scan
            Self::scan_directory_inner_node(
                &provider,
                &registry,
                &events_sender,
                node,
                session,
                pipeline_config,
                &cancel,
            ).await;

            // Clean up
            let _ = active_scans.remove_async(&session).await;
        });
    }

    /// Inner scan logic (static, doesn't need &self)
    async fn scan_directory_inner_node(
        provider: &Arc<dyn FsProvider>,
        registry: &NodeRegistry,
        events_sender: &Sender<Event>,
        node: NodeId,
        session: SessionId,
        pipeline_config: PipelineConfig,
        cancel: &CancellationToken,
    ) {
        let Some(path) = registry.resolve(node) else {
            debug_assert!(true);
            let _ = events_sender.send(Event::Error { message: format!("Unable to resolve ID: {node:?}"), recoverable: false, session });
            return;
        };
        // 1. List directory
        let entries = match provider.list(&path).await {
            Ok(entries) => entries,
            Err(e) => {
                let _ = events_sender
                    .send_async(Event::Error {
                        message: format!("Failed to scan {}: {}", path.display(), e),
                        recoverable: true,
                        session,
                    })
                    .await;
                return;
            }
        };

        // 2. Check cancellation
        if cancel.is_cancelled() {
            return;
        }

        // 3. Register nodes
        let parent_id = registry.clone().register(path.clone());
        registry.clone().register_batch_file_node(&entries);

        // 4. Execute pipeline
        let pipeline = Pipeline::from_config(&pipeline_config);
        let processed = pipeline.execute(entries);

        let final_entries = match processed {
            PipelineData::Flat(file_nodes) => file_nodes,
            PipelineData::Grouped(grouped_nodes) => grouped_nodes
                .groups
                .into_iter()
                .flat_map(|g| g.nodes)
                .collect(),
        };

        // 5. Check cancellation again
        if cancel.is_cancelled() {
            return;
        }

        // 6. Send result
        let _ = events_sender
            .send_async(Event::DirectoryLoaded {
                parent: parent_id,
                path: path.to_path_buf(),
                entries: final_entries,
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
                Ok(ScanCommand::Scan { path, session, pipeline }) => {
                    // Clone what we need and spawn - doesn't block the command loop
                    Self::spawn_scan(
                        self.provider.clone(),
                        self.registry.clone(),
                        self.events_sender.clone(),
                        self.active_scans.clone(),
                        path,
                        session,
                        pipeline,
                    );
                }
                Ok(ScanCommand::ScanNode { node, session, pipeline }) => {
                    Self::spawn_scan_node(
                        self.provider.clone(),
                        self.registry.clone(),
                        self.events_sender.clone(),
                        self.active_scans.clone(),
                        node,
                        session,
                        pipeline,
                    );
                }
                Ok(ScanCommand::Cancel(session)) => {
                    self.cancel_scan(session).await;
                }
                Err(_) | Ok(ScanCommand::Shutdown) => {
                    self.active_scans.iter_async(|_k, v| {
                        v.cancel();
                        true
                    }).await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "scanner"
    }
}