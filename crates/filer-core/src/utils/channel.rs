use std::future::Future;
use std::pin::Pin;

use flume::Sender;

use crate::api::event_sink::EventSink;
use crate::api::events::Event;

pub trait SyncSend<T> {
    fn send_value(&self, value: T) -> Result<(), ()>;
}

pub trait AsyncSend<T> {
    fn send_value_async<'a>(
        &'a self,
        value: T,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>>;
}

impl<T> SyncSend<T> for Sender<T> {
    fn send_value(&self, value: T) -> Result<(), ()> {
        self.send(value).map_err(|_| ())
    }
}

impl<T: Send> AsyncSend<T> for Sender<T> {
    fn send_value_async<'a>(
        &'a self,
        value: T,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
        Box::pin(async move { self.send_async(value).await.map_err(|_| ()) })
    }
}

impl SyncSend<Event> for EventSink {
    fn send_value(&self, value: Event) -> Result<(), ()> {
        self.send(value).map_err(|_| ())
    }
}

impl AsyncSend<Event> for EventSink {
    fn send_value_async<'a>(
        &'a self,
        value: Event,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
        Box::pin(async move { self.send_async(value).await.map_err(|_| ()) })
    }
}

/// Send a value on a channel, logging a warning if the channel is closed.
///
/// This replaces the `let _ = tx.send(val)` pattern that silently
/// discards send failures. Use this whenever dropping a message
/// would be unexpected (actor channels, event bus).
///
/// ```ignore
/// use filer_core::utils::channel::send_or_warn;
/// send_or_warn(&event_tx, Event::SessionCreated(id), "emit SessionCreated");
/// ```
pub fn send_or_warn<T, S>(tx: &S, val: T, context: &str)
where
    T: std::fmt::Debug,
    S: SyncSend<T>,
{
    if tx.send_value(val).is_err() {
        tracing::warn!(context = context, "channel send failed");
    }
}

/// Async variant of [`send_or_warn`] for use inside `async` contexts.
pub async fn send_or_warn_async<T, S>(tx: &S, val: T, context: &str)
where
    T: std::fmt::Debug,
    S: AsyncSend<T>,
{
    if tx.send_value_async(val).await.is_err() {
        tracing::warn!(context = context, "async channel send failed");
    }
}
