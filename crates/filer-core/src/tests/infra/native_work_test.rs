use std::time::Duration;

use tokio::time::timeout;

use crate::actors::native;

#[tokio::test]
async fn dropping_the_calling_future_does_not_release_native_work() {
    let scope = native::Scope::default();
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, held) = flume::bounded::<()>(1);

    scope
        .enter()
        .run(async {
            let handle = native::spawn_blocking(move || {
                entered.send(()).unwrap();
                let _ = held.recv();
            });
            started.await.unwrap();
            drop(handle);
        })
        .await;

    assert!(
        timeout(Duration::from_millis(25), scope.wait())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    timeout(Duration::from_secs(5), scope.wait()).await.unwrap();
}

struct NativeRead {
    session: crate::model::session::SessionId,
    entered: tokio::sync::oneshot::Sender<()>,
    released: flume::Receiver<()>,
    cancelled: crate::CancelSignal,
}

impl crate::Module for NativeRead {
    fn init(self: Box<Self>, ctx: crate::api::module::ModuleContext<'_>) {
        let Self {
            session,
            entered,
            released,
            cancelled,
        } = *self;
        assert!(
            ctx.actors
                .work_tracker()
                .spawn_read(session, cancelled, async {
                    native::spawn_blocking(move || {
                        entered.send(()).unwrap();
                        released.recv().unwrap();
                    })
                    .await
                    .unwrap();
                })
        );
    }
}

#[tokio::test]
async fn session_close_cancels_the_read_and_waits_for_its_native_work() {
    use crate::{CancelSignal, Command, Event, FilerCore};

    let core = FilerCore::new();
    let events = core.event_receiver();
    let session = core.sessions().create_session(core.event_sender());
    let cancelled = CancelSignal::new();
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, released) = flume::bounded(1);
    core.load(NativeRead {
        session,
        entered,
        released,
        cancelled: cancelled.clone(),
    });
    let limit = Duration::from_secs(5);
    timeout(limit, started).await.unwrap().unwrap();

    core.send(Command::DestroySession(session)).unwrap();
    timeout(limit, cancelled.cancelled()).await.unwrap();
    assert!(core.sessions().exists(session));
    assert!(
        timeout(Duration::from_millis(25), events.recv_async())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    assert!(
        matches!(timeout(limit, events.recv_async()).await.unwrap().unwrap(),
        Event::SessionDestroyed(actual) if actual == session)
    );
    core.shutdown().await.unwrap();
}
