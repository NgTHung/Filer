pub mod cancel;
pub mod router;

use tokio::task::JoinHandle;

/// Trait for all actors in the system.
///
/// Actors are long-lived concurrent tasks that communicate via channels.
/// Each actor owns a `Receiver` for incoming commands and a `Sender` for
/// outgoing events. The `run()` method contains the actor's main loop.
///
/// # Implementing custom actors
///
/// Modules implement this trait for their internal actors and spawn
/// them via `ModuleContext::actors`:
///
/// ```ignore
/// struct MyActor { events: Receiver<Event> }
///
/// impl Actor for MyActor {
///     async fn run(self) {
///         while let Ok(e) = self.events.recv_async().await {
///             // react to events
///         }
///     }
///     fn name(&self) -> &'static str { "my-actor" }
/// }
/// ```
pub trait Actor: Send + 'static {
    /// Start the actor's main loop (consumes self)
    fn run(self) -> impl std::future::Future<Output = ()> + Send;

    /// Get actor name for logging
    fn name(&self) -> &'static str;
}

/// Manages spawned actor tasks with uniform lifecycle control.
///
/// All actors — both built-in and custom — are spawned through `ActorSystem`.
/// It tracks their `JoinHandle`s so that `shutdown()` can abort them all,
/// causing a cascading channel-close that cleanly tears down the pipeline.
pub struct ActorSystem {
    handles: std::sync::Mutex<Vec<JoinHandle<()>>>,
}

impl ActorSystem {
    pub fn new() -> Self {
        Self {
            handles: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Spawn an actor onto the tokio runtime.
    ///
    /// The actor's `run()` future executes in a new tokio task.
    /// The `JoinHandle` is tracked for shutdown.
    pub fn spawn<A: Actor>(&self, actor: A) {
        let name = actor.name();
        let handle = tokio::spawn(async move {
            tracing::info!(actor = name, "started");
            actor.run().await;
            tracing::info!(actor = name, "stopped");
        });
        self.handles.lock().unwrap().push(handle);
    }

    /// Abort all running actor tasks.
    ///
    /// Aborting causes each task to drop its actor, which drops all owned
    /// channel senders/receivers. This cascades: downstream actors see
    /// their receive channels close and exit their loops naturally.
    pub fn shutdown(&self) {
        for handle in self.handles.lock().unwrap().iter() {
            handle.abort();
        }
    }

    /// Number of actors currently tracked.
    pub fn actor_count(&self) -> usize {
        self.handles.lock().unwrap().len()
    }
}
