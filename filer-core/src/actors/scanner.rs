use std::path::PathBuf;
use flume::{Receiver, Sender};

use crate::actors::Actor;
use crate::api::events::Event;
use crate::vfs::provider::FsProvider;

/// Commands for scanner actor
#[derive(Debug, Clone)]
pub enum ScanCommand {
    Scan { path: PathBuf, depth: Option<usize> },
    Cancel,
}

/// Scanner actor - handles directory traversal
pub struct Scanner {
    commands: Receiver<ScanCommand>,
    events: Sender<Event>,
    provider: Box<dyn FsProvider>,
}

impl Scanner {
    pub fn new(
        commands: Receiver<ScanCommand>,
        events: Sender<Event>,
        provider: Box<dyn FsProvider>,
    ) -> Self {
        todo!()
    }
    
    async fn scan_directory(&self, path: &PathBuf, depth: Option<usize>) {
        todo!()
    }
}

impl Actor for Scanner {
    async fn run(self) {
        todo!()
    }
    
    fn name(&self) -> &'static str {
        "scanner"
    }
}