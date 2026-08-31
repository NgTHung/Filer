use super::*;

#[tokio::test]
async fn synthetic_burst_preserves_order_and_invalidates_each_location_once() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let (nav_tx, nav_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let temp_dir = TempDir::new().unwrap();

    let create_path = temp_dir.path().join("create-root");
    let delete_path = temp_dir.path().join("delete-root");
    let rename_path = temp_dir.path().join("rename-root");
    let create_location = LocationRef::from_location(&Location::local(create_path.clone()));
    let delete_location = LocationRef::from_location(&Location::local(delete_path.clone()));
    let rename_location = LocationRef::from_location(&Location::local(rename_path.clone()));
    let rename_from = rename_path.join("old-name.txt");

    let watcher = Watcher::with_refresh(cmd_rx, evt_tx, registry, provider.clone(), nav_tx);
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    for location in [
        create_location.clone(),
        delete_location.clone(),
        rename_location.clone(),
    ] {
        cmd_tx.send(location_watch(location, SessionId(1))).unwrap();
    }

    timeout(Duration::from_secs(1), async {
        loop {
            if provider.watched_paths.lock().unwrap().len() == 3 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all burst locations should be registered");

    provider
        .emit(create_path.join("new.txt"), FsChangeKind::Created)
        .await;
    provider
        .emit(delete_path.join("old.txt"), FsChangeKind::Deleted)
        .await;
    provider
        .emit(
            rename_path.join("new-name.txt"),
            FsChangeKind::Renamed {
                from: rename_from.clone(),
            },
        )
        .await;

    let events = collect_events(&evt_rx, 3, 1000).await;
    assert_eq!(
        events.len(),
        3,
        "the full synthetic burst should be emitted"
    );
    assert!(matches!(
        &events[0],
        Event::FsChanged {
            location,
            kind: FsChangeKind::Created,
            session: SessionId(1),
        } if location == &create_location
    ));
    assert!(matches!(
        &events[1],
        Event::FsChanged {
            location,
            kind: FsChangeKind::Deleted,
            session: SessionId(1),
        } if location == &delete_location
    ));
    assert!(matches!(
        &events[2],
        Event::FsChanged {
            location,
            kind: FsChangeKind::Renamed { from },
            session: SessionId(1),
        } if location == &rename_location && from == &rename_from
    ));
    assert!(
        evt_rx.try_recv().is_err(),
        "the burst should not emit duplicate filesystem events"
    );

    let invalidations = [
        timeout(Duration::from_secs(1), nav_rx.recv_async())
            .await
            .expect("create should invalidate navigation")
            .expect("navigation channel should remain open"),
        timeout(Duration::from_secs(1), nav_rx.recv_async())
            .await
            .expect("delete should invalidate navigation")
            .expect("navigation channel should remain open"),
        timeout(Duration::from_secs(1), nav_rx.recv_async())
            .await
            .expect("rename should invalidate navigation")
            .expect("navigation channel should remain open"),
    ];
    assert!(matches!(
        &invalidations[0],
        NavCommand::Invalidate(location) if location == &create_location
    ));
    assert!(matches!(
        &invalidations[1],
        NavCommand::Invalidate(location) if location == &delete_location
    ));
    assert!(matches!(
        &invalidations[2],
        NavCommand::Invalidate(location) if location == &rename_location
    ));
    assert!(
        nav_rx.try_recv().is_err(),
        "each watched location should invalidate exactly once"
    );

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}
