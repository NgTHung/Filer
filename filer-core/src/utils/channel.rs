use flume::Sender;

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
pub fn send_or_warn<T: std::fmt::Debug>(tx: &Sender<T>, val: T, context: &str) {
    if let Err(e) = tx.send(val) {
        tracing::warn!(context = context, "channel send failed: {e}");
    }
}

/// Async variant of [`send_or_warn`] for use inside `async` contexts.
pub async fn send_or_warn_async<T: std::fmt::Debug>(tx: &Sender<T>, val: T, context: &str) {
    if let Err(e) = tx.send_async(val).await {
        tracing::warn!(context = context, "async channel send failed: {e}");
    }
}
