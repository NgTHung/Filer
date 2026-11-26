//! # Runtime shutdown
//!
//! The runtime owns the shutdown sequence so a cancelled caller can resume
//! waiting while other callers continue operating a paused mutation queue.
//!
//! ```
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), filer_core::CoreError> {
//! let core = filer_core::FilerCore::new();
//! let events = core.event_receiver();
//! let consume = async { while events.recv_async().await.is_ok() {} };
//! let (result, ()) = tokio::join!(core.shutdown(), consume);
//! result
//! # }
//! ```

use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use crate::actors::ActorSystem;
use crate::api::event_sink::{EventDelivery, EventSink};
use crate::{CancelSignal, CoreError};

type Completion = Option<Result<(), Arc<CoreError>>>;

pub(crate) struct Shutdown {
    requested: CancelSignal,
    router_done: flume::Receiver<Result<(), CoreError>>,
    pending: Mutex<Option<(EventDelivery, watch::Sender<Completion>)>>,
    completed: watch::Receiver<Completion>,
}

pub(crate) struct RouterShutdown {
    pub(crate) requested: CancelSignal,
    pub(crate) done: flume::Sender<Result<(), CoreError>>,
}

impl Shutdown {
    pub(crate) fn new(delivery: EventDelivery) -> (Self, RouterShutdown) {
        let requested = CancelSignal::new();
        let (done, router_done) = flume::bounded(1);
        let (complete, completed) = watch::channel(None);
        (
            Self {
                requested: requested.clone(),
                router_done,
                pending: Mutex::new(Some((delivery, complete))),
                completed,
            },
            RouterShutdown { requested, done },
        )
    }

    pub(crate) async fn run(
        &self,
        system: &Arc<ActorSystem>,
        sink: &EventSink,
    ) -> Result<(), CoreError> {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some((delivery, complete)) = pending {
            self.requested.cancel();
            let router_done = self.router_done.clone();
            let system = system.clone();
            let sink = sink.clone();
            tokio::spawn(async move {
                let closed = router_done.recv_async().await.unwrap_or_else(|_| {
                    Err(CoreError::actor(
                        "command router",
                        "Session closure did not finish",
                    ))
                });
                let stopped = system.shutdown().await;
                let delivered = delivery.finish(&sink).await;
                complete.send_replace(Some(closed.and(stopped).and(delivered).map_err(Arc::new)));
            });
        }

        let mut completed = self.completed.clone();
        let result = completed
            .wait_for(Option::is_some)
            .await
            .map_err(|_| CoreError::actor("runtime shutdown", "Completion task stopped"))?;
        match result.as_ref() {
            Some(Ok(())) => Ok(()),
            Some(Err(error)) => {
                Err(CoreError::actor("runtime shutdown", error.to_string())
                    .with_source(error.clone()))
            }
            None => Err(CoreError::actor(
                "runtime shutdown",
                "Missing completion result",
            )),
        }
    }
}
