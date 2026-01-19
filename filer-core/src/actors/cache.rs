use std::collections::HashMap;

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
    entries: HashMap<NodeId, FileNode>,
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