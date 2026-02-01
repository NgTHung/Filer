pub mod channel;

use flume::{Receiver, Sender};
use std::any::{Any, TypeId};
use std::sync::Arc;

use crate::CoreError;
use crate::bus::channel::Channel;

/// Central message bus for actor communication
pub struct EventBus {
    channels: Arc<scc::HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            channels: Arc::clone(&self.channels),
        }
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(scc::HashMap::new()),
        }
    }

    /// Register a new channel for a message type
    pub fn register<M: Clone + Send + 'static>(&self, capacity: usize) -> Sender<M> {
        let ti = TypeId::of::<M>();
        let ch = channel::Channel::bounded(capacity);
        let ret = ch.tx.clone();
        self.channels.insert_sync(ti, Box::new(ch)).ok();
        ret
    }

    /// Subscribe to a message type
    pub fn subscribe<M: Clone + Send + 'static>(&self) -> Option<Receiver<M>> {
        let ti = TypeId::of::<M>();
        self.channels.read_sync(&ti, |_, v| {
            v.downcast_ref::<Channel<M>>()
                .map(|ch| ch.rx.clone())
        }).flatten()
    }

    /// Publish a message
    pub fn publish<M: Clone + Send + 'static>(&self, message: M) -> Result<(), CoreError> {
        let ti = TypeId::of::<M>();

        self.channels
            .read_sync(&ti, |_, v| {
                v.downcast_ref::<Channel<M>>()
                    .ok_or(CoreError::ChannelClosed)?
                    .tx
                    .send(message.clone())
                    .map_err(|e| CoreError::ChannelError(e.to_string()))
            })
            .unwrap_or(Err(CoreError::ChannelClosed))
    }
}
