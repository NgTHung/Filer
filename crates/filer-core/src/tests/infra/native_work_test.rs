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
