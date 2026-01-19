pub mod channel;

use flume::{Receiver, Sender};
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Central message bus for actor communication
pub struct EventBus {
    channels: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl EventBus {
    pub fn new() -> Self {
        todo!()
    }
    
    /// Register a new channel for a message type
    pub fn register<M: Clone + Send + 'static>(&mut self, capacity: usize) -> Sender<M> {
        todo!()
    }
    
    /// Subscribe to a message type
    pub fn subscribe<M: Clone + Send + 'static>(&self) -> Option<Receiver<M>> {
        todo!()
    }
    
    /// Publish a message
    pub fn publish<M: Clone + Send + 'static>(&self, message: M) {
        todo!()
    }
}