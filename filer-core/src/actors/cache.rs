use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::actors::Actor;
use crate::model::node::{FileNode, NodeId};

/// Commands for cache actor
#[derive(Debug, Clone)]
pub enum CacheCommand {
    Store(FileNode),
    StoreBatch(Vec<FileNode>),
    Get(NodeId),
    Invalidate(NodeId),
    Clear,
}

/// Cache actor - LRU cache for file nodes
pub struct Cache {
    commands: flume::Receiver<CacheCommand>,
    entries: Arc<scc::HashMap<NodeId, FileNode, RandomState>>,
    capacity: usize,
}

impl Cache {
    pub fn new(commands: flume::Receiver<CacheCommand>, capacity: usize) -> Self {
        todo!()
    }
}

impl Actor for Cache {
    async fn run(self) {
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "cache"
    }
}