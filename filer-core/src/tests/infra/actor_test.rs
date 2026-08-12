use crate::actors::cancel::CancellationToken;
use crate::actors::{Actor, WorkTracker};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Simple test actor for trait testing
struct TestActor {
    name: &'static str,
    executed: Arc<Mutex<bool>>,
}

impl TestActor {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            executed: Arc::new(Mutex::new(false)),
        }
    }

    fn _was_executed(&self) -> bool {
        *self.executed.lock().unwrap()
    }
}

impl Actor for TestActor {
    async fn run(self) {
        *self.executed.lock().unwrap() = true;
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

#[cfg(test)]
mod actor_trait_tests {
    use super::*;

    #[tokio::test]
    async fn test_actor_trait_name() {
        let actor = TestActor::new("test-actor");
        assert_eq!(actor.name(), "test-actor");
    }

    #[tokio::test]
    async fn test_actor_trait_run() {
        let actor = TestActor::new("test-runner");
        let executed = Arc::clone(&actor.executed);

        assert!(!*executed.lock().unwrap());

        actor.run().await;

        assert!(*executed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_actor_is_send() {
        let actor = TestActor::new("send-test");

        // Spawn in tokio task to verify Send trait
        let handle = tokio::spawn(async move {
            actor.run().await;
        });

        handle.await.expect("Actor should run in tokio task");
    }

    #[tokio::test]
    async fn test_multiple_actors_concurrent() {
        let actor1 = TestActor::new("actor-1");
        let actor2 = TestActor::new("actor-2");

        let exec1 = Arc::clone(&actor1.executed);
        let exec2 = Arc::clone(&actor2.executed);

        // Run actors concurrently
        let handle1 = tokio::spawn(async move { actor1.run().await });
        let handle2 = tokio::spawn(async move { actor2.run().await });

        handle1.await.expect("Actor 1 failed");
        handle2.await.expect("Actor 2 failed");

        assert!(*exec1.lock().unwrap());
        assert!(*exec2.lock().unwrap());
    }

    #[tokio::test]
    async fn test_actor_spawn_multiple_times() {
        let actor = TestActor::new("respawnable");
        let executed = Arc::clone(&actor.executed);

        // Run same actor type multiple times
        actor.run().await;

        let actor2 = TestActor::new("respawnable");
        actor2.run().await;

        assert!(*executed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_actor_name_immutable() {
        let actor = TestActor::new("immutable-name");
        let name1 = actor.name();
        let name2 = actor.name();

        assert_eq!(name1, name2);
        assert_eq!(name1, "immutable-name");
    }

    #[tokio::test]
    async fn tracked_work_is_cancelled_and_joined_before_shutdown_returns() {
        let tracker = WorkTracker::new();
        let token = CancellationToken::new();
        let completed = Arc::new(AtomicBool::new(false));
        let task_completed = completed.clone();
        assert!(tracker.spawn(token.clone(), async move {
            token.cancelled().await;
            tokio::task::yield_now().await;
            task_completed.store(true, Ordering::SeqCst);
        }));

        tracker.shutdown().await.unwrap();

        assert!(completed.load(Ordering::SeqCst));
        assert!(!tracker.spawn(CancellationToken::new(), async {}));
    }
}
