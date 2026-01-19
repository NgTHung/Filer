use flume::{Receiver, Sender};

/// Typed channel pair
pub struct Channel<T> {
    pub tx: Sender<T>,
    pub rx: Receiver<T>,
}

impl<T> Channel<T> {
    pub fn bounded(capacity: usize) -> Self {
        let (tx, rx) = flume::bounded(capacity);
        Self { tx, rx }
    }
    
    pub fn unbounded() -> Self {
        let (tx, rx) = flume::unbounded();
        Self { tx, rx }
    }
}