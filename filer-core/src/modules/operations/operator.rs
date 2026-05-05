use std::path::{Path, PathBuf};
use std::sync::Arc;

use flume::{Receiver, Sender};

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::{Event, OperationKind};
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::{CoreError, FsProvider};

#[derive(Debug, Clone)]
pub enum OpsCommand {
    Copy {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
    },
    Move {
        sources: Vec<NodeId>,
        destination: NodeId,
        session: SessionId,
    },
    Delete {
        targets: Vec<NodeId>,
        trash: bool,
        session: SessionId,
    },
    Rename {
        source: NodeId,
        new_name: String,
        session: SessionId,
    },
    CreateFolder {
        parent: NodeId,
        name: String,
        session: SessionId,
    },
    CreateFile {
        parent: NodeId,
        name: String,
        session: SessionId,
    },
    Cancel(SessionId),
}

pub struct Operator {
    commands: Receiver<OpsCommand>,
    events: Sender<Event>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_ops: CancelMap,
    trash_fn: Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>,
    cache: Option<SharedDirCache>,
}

impl Operator {
    pub fn new(
        commands: Receiver<OpsCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self::with_trash_fn(
            commands,
            events,
            provider,
            registry,
            Arc::new(|path| {
                trash::delete(path).map_err(|e| CoreError::Io {
                    path: path.to_path_buf(),
                    message: e.to_string(),
                })
            }),
        )
    }

    pub fn with_trash_fn(
        commands: Receiver<OpsCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        trash_fn: Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>,
    ) -> Self {
        Self {
            commands,
            events,
            provider,
            registry,
            active_ops: CancelMap::new(),
            trash_fn,
            cache: None,
        }
    }

    pub fn with_cache(
        commands: Receiver<OpsCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        let mut op = Self::new(commands, events, provider, registry);
        op.cache = Some(cache);
        op
    }

    #[allow(dead_code)]
    fn invalidate_parent(&self, path: &Path) {
        invalidate_parent_cache(&self.cache, path);
    }

    fn copy(&self, sources: Vec<NodeId>, dest: NodeId, session: SessionId) {
        let Some(dst_path) = self.registry.resolve(dest) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve destination {dest:?}"),
                    recoverable: true,
                    session,
                },
                "operator: copy resolve dest",
            );
            return;
        };

        let mut src_paths = Vec::new();
        for src_id in &sources {
            let Some(path) = self.registry.resolve(*src_id) else {
                send_or_warn(
                    &self.events,
                    Event::Error {
                        message: format!("Cannot resolve source {src_id:?}"),
                        recoverable: true,
                        session,
                    },
                    "operator: copy resolve src",
                );
                return;
            };
            src_paths.push(path);
        }

        let cancel = self.active_ops.arm(session);
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            let mut affected = Vec::new();

            for src_path in src_paths {
                if cancel.is_cancelled() {
                    return;
                }

                let Ok(meta) = fs.metadata(&src_path).await else {
                    send_or_warn_async(
                        &events,
                        Event::Error {
                            message: format!("Cannot stat {}", src_path.display()),
                            recoverable: true,
                            session,
                        },
                        "operator: copy stat",
                    )
                    .await;
                    return;
                };

                let file_name = src_path.file_name().unwrap_or_default();

                if meta.is_dir() {
                    let dst_sub = dst_path.join(file_name);
                    let mut items_done = 0usize;
                    match copy_dir_recursive(
                        &fs,
                        &src_path,
                        &dst_sub,
                        &cancel,
                        &events,
                        &registry,
                        session,
                        &mut items_done,
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(CoreError::Cancelled) => return,
                        Err(e) => {
                            send_or_warn_async(
                                &events,
                                Event::from_error(e, session),
                                "operator: copy dir",
                            )
                            .await;
                            return;
                        }
                    }
                    invalidate_parent_cache(&cache, &dst_sub);
                    affected.push(registry.clone().register(dst_sub));
                } else {
                    let dst_file = dst_path.join(file_name);
                    if let Err(e) = fs.copy(&src_path, &dst_file).await {
                        send_or_warn_async(
                            &events,
                            Event::from_error(e, session),
                            "operator: copy file",
                        )
                        .await;
                        return;
                    }
                    invalidate_parent_cache(&cache, &dst_file);
                    affected.push(registry.clone().register(dst_file));
                }
            }

            send_or_warn_async(
                &events,
                Event::OperationComplete {
                    operation: OperationKind::Copy,
                    success: true,
                    affected,
                    session,
                },
                "operator: copy complete",
            )
            .await;
        });
    }

    fn moves(&self, sources: Vec<NodeId>, dest: NodeId, session: SessionId) {
        let Some(dst_path) = self.registry.resolve(dest) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve destination {dest:?}"),
                    recoverable: true,
                    session,
                },
                "operator: move resolve dest",
            );
            return;
        };

        let mut src_paths = Vec::new();
        for src_id in &sources {
            let Some(path) = self.registry.resolve(*src_id) else {
                send_or_warn(
                    &self.events,
                    Event::Error {
                        message: format!("Cannot resolve source {src_id:?}"),
                        recoverable: true,
                        session,
                    },
                    "operator: move resolve src",
                );
                return;
            };
            src_paths.push(path);
        }

        let cancel = self.active_ops.arm(session);
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            let mut affected = Vec::new();

            for src_path in src_paths {
                if cancel.is_cancelled() {
                    return;
                }

                let file_name = src_path.file_name().unwrap_or_default();
                let dst_file = dst_path.join(file_name);

                match fs.rename(&src_path, &dst_file).await {
                    Ok(()) => {
                        invalidate_parent_cache(&cache, &src_path);
                        invalidate_parent_cache(&cache, &dst_file);
                        affected.push(registry.clone().register(dst_file));
                    }
                    Err(e) if is_cross_device(&e) => {
                        if let Err(e) = fs.copy(&src_path, &dst_file).await {
                            send_or_warn_async(
                                &events,
                                Event::from_error(e, session),
                                "operator: move copy",
                            )
                            .await;
                            return;
                        }
                        if let Err(e) = fs.delete(&src_path).await {
                            send_or_warn_async(
                                &events,
                                Event::from_error(e, session),
                                "operator: move delete",
                            )
                            .await;
                            return;
                        }
                        invalidate_parent_cache(&cache, &src_path);
                        invalidate_parent_cache(&cache, &dst_file);
                        affected.push(registry.clone().register(dst_file));
                    }
                    Err(e) => {
                        send_or_warn_async(
                            &events,
                            Event::from_error(e, session),
                            "operator: move rename",
                        )
                        .await;
                        return;
                    }
                }
            }

            send_or_warn_async(
                &events,
                Event::OperationComplete {
                    operation: OperationKind::Move,
                    success: true,
                    affected,
                    session,
                },
                "operator: move complete",
            )
            .await;
        });
    }

    fn delete(&self, targets: Vec<NodeId>, trash: bool, session: SessionId) {
        let mut paths: Vec<(NodeId, PathBuf)> = Vec::new();
        for target in &targets {
            let Some(path) = self.registry.resolve(*target) else {
                send_or_warn(
                    &self.events,
                    Event::Error {
                        message: format!("Cannot resolve node {target:?}"),
                        recoverable: true,
                        session,
                    },
                    "operator: delete resolve",
                );
                return;
            };
            paths.push((*target, path));
        }

        let cancel = self.active_ops.arm(session);
        let events = self.events.clone();
        let fs = self.provider.clone();
        let trash_fn = self.trash_fn.clone();
        let cache = self.cache.clone();
        let total = paths.len();

        tokio::spawn(async move {
            let mut affected = Vec::new();
            let mut items_done = 0usize;

            for (id, path) in paths {
                if cancel.is_cancelled() {
                    return;
                }

                let result = if trash {
                    let tf = trash_fn.clone();
                    let p = path.clone();
                    tokio::task::spawn_blocking(move || tf(&p))
                        .await
                        .unwrap_or_else(|e| {
                            Err(CoreError::ActorError {
                                actor: "operator",
                                message: e.to_string(),
                            })
                        })
                } else {
                    fs.delete(&path).await
                };

                match result {
                    Ok(()) => {
                        invalidate_parent_cache(&cache, &path);
                        affected.push(id);
                        items_done += 1;
                        if total > 1 {
                            send_or_warn_async(
                                &events,
                                Event::OperationProgress {
                                    operation: OperationKind::Delete,
                                    total_items: total,
                                    items_done,
                                    current_file: id,
                                    session,
                                },
                                "operator: delete progress",
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        send_or_warn_async(
                            &events,
                            Event::from_error(e, session),
                            "operator: delete error",
                        )
                        .await;
                        return;
                    }
                }
            }

            send_or_warn_async(
                &events,
                Event::OperationComplete {
                    operation: OperationKind::Delete,
                    success: true,
                    affected,
                    session,
                },
                "operator: delete complete",
            )
            .await;
        });
    }

    fn rename(&self, source: NodeId, new_name: String, session: SessionId) {
        let Some(src_path) = self.registry.resolve(source) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve node {source:?}"),
                    recoverable: true,
                    session,
                },
                "operator: rename resolve",
            );
            return;
        };

        let Some(parent) = src_path.parent() else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot get parent of {}", src_path.display()),
                    recoverable: true,
                    session,
                },
                "operator: rename parent",
            );
            return;
        };

        let new_path = parent.join(&new_name);
        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            if fs.exists(&new_path).await.unwrap_or(true) {
                send_or_warn_async(
                    &events,
                    Event::Error {
                        message: "File/Folder already exists".to_string(),
                        recoverable: true,
                        session,
                    },
                    "operator: rename collision",
                )
                .await;
                return;
            }

            if let Err(e) = fs.rename(&src_path, &new_path).await {
                send_or_warn_async(&events, Event::from_error(e, session), "operator: rename")
                    .await;
                return;
            }

            invalidate_parent_cache(&cache, &src_path);
            let id = registry.register(new_path);
            send_or_warn_async(
                &events,
                Event::OperationComplete {
                    operation: OperationKind::Rename,
                    success: true,
                    affected: vec![id],
                    session,
                },
                "operator: rename complete",
            )
            .await;
        });
    }

    fn create_file(&self, parent: NodeId, name: String, session: SessionId) {
        let Some(path) = self.registry.resolve(parent) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve node {parent:?}"),
                    recoverable: true,
                    session,
                },
                "operator: create_file resolve",
            );
            return;
        };

        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            let full_path = path.join(name);
            if fs.exists(&full_path).await.unwrap_or(true) {
                send_or_warn_async(
                    &events,
                    Event::Error {
                        message: "File/Folder already exists".to_string(),
                        recoverable: true,
                        session,
                    },
                    "operator: create_file exists",
                )
                .await;
                return;
            }
            if let Err(e) = fs.write(&full_path, &[]).await {
                send_or_warn_async(
                    &events,
                    Event::from_error(e, session),
                    "operator: create_file write",
                )
                .await;
                return;
            }
            invalidate_parent_cache(&cache, &full_path);
            let id = registry.register(full_path);
            send_or_warn_async(
                &events,
                Event::OperationComplete {
                    operation: OperationKind::CreateFile,
                    success: true,
                    affected: vec![id],
                    session,
                },
                "operator: create_file complete",
            )
            .await;
        });
    }

    fn create_folder(&self, parent: NodeId, name: String, session: SessionId) {
        let Some(path) = self.registry.resolve(parent) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve node {parent:?}"),
                    recoverable: true,
                    session,
                },
                "operator: create_folder resolve",
            );
            return;
        };

        let events = self.events.clone();
        let registry = self.registry.clone();
        let fs = self.provider.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            let full_path = path.join(name);
            if fs.exists(&full_path).await.unwrap_or(true) {
                send_or_warn_async(
                    &events,
                    Event::Error {
                        message: "File/Folder already exists".to_string(),
                        recoverable: true,
                        session,
                    },
                    "operator: create_folder exists",
                )
                .await;
                return;
            }
            if let Err(e) = fs.mkdir(&full_path).await {
                send_or_warn_async(
                    &events,
                    Event::from_error(e, session),
                    "operator: create_folder mkdir",
                )
                .await;
                return;
            }
            invalidate_parent_cache(&cache, &full_path);
            let id = registry.register(full_path);
            send_or_warn_async(
                &events,
                Event::OperationComplete {
                    operation: OperationKind::CreateFolder,
                    success: true,
                    affected: vec![id],
                    session,
                },
                "operator: create_folder complete",
            )
            .await;
        });
    }
}

async fn copy_dir_recursive(
    fs: &Arc<dyn FsProvider>,
    src: &Path,
    dst: &Path,
    cancel: &CancellationToken,
    events: &Sender<Event>,
    registry: &NodeRegistry,
    session: SessionId,
    items_done: &mut usize,
) -> Result<(), CoreError> {
    fs.mkdir(dst).await?;
    let entries = fs.list(src).await?;
    for entry in entries {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let src_child = src.join(&entry.name);
        let dst_child = dst.join(&entry.name);
        if entry.is_dir() {
            Box::pin(copy_dir_recursive(
                fs, &src_child, &dst_child, cancel, events, registry, session, items_done,
            ))
            .await?;
        } else {
            fs.copy(&src_child, &dst_child).await?;
            *items_done += 1;
            let id = registry.clone().register(dst_child);
            send_or_warn_async(
                events,
                Event::OperationProgress {
                    operation: OperationKind::Copy,
                    total_items: 0,
                    items_done: *items_done,
                    current_file: id,
                    session,
                },
                "operator: copy dir progress",
            )
            .await;
        }
    }
    Ok(())
}

fn is_cross_device(err: &CoreError) -> bool {
    matches!(err, CoreError::Io { message, .. }
        if message.contains("cross-device")
            || message.contains("os error 18")
            || message.contains("os error 17"))
}

fn invalidate_parent_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let (Some(parent), Some(c)) = (path.parent(), cache) {
        if let Ok(mut guard) = c.lock() {
            guard.invalidate(parent);
        }
    }
}

impl Actor for Operator {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Err(_) => {
                    self.active_ops.cancel_all().await;
                    break;
                }
                Ok(OpsCommand::Cancel(s)) => self.active_ops.cancel(s),
                Ok(OpsCommand::Copy {
                    sources,
                    destination,
                    session,
                }) => self.copy(sources, destination, session),
                Ok(OpsCommand::Move {
                    sources,
                    destination,
                    session,
                }) => self.moves(sources, destination, session),
                Ok(OpsCommand::Delete {
                    targets,
                    trash,
                    session,
                }) => self.delete(targets, trash, session),
                Ok(OpsCommand::Rename {
                    source,
                    new_name,
                    session,
                }) => self.rename(source, new_name, session),
                Ok(OpsCommand::CreateFile {
                    parent,
                    name,
                    session,
                }) => self.create_file(parent, name, session),
                Ok(OpsCommand::CreateFolder {
                    parent,
                    name,
                    session,
                }) => self.create_folder(parent, name, session),
            }
        }
    }

    fn name(&self) -> &'static str {
        "operator"
    }
}
