use filer_task_web::storage::{Storage, StorageError};

#[tokio::test]
async fn identities_are_unique_stable_resolvable_and_persistent() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let path = temp.path().join("state.sqlite3");
    let storage = Storage::open(&path).await.expect("storage opens");
    let created = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("identity creates");
    assert_eq!(created.identity.username, "Alice");
    assert_eq!(created.session_token.len(), 64);
    storage
        .create_identity("Straße", "Test browser")
        .await
        .expect("second identity creates");
    assert!(matches!(
        storage.create_identity("STRAßE", "Test browser").await,
        Err(StorageError::UsernameTaken)
    ));

    let renamed = storage
        .rename_identity(created.identity.user_id, "Bob")
        .await
        .expect("identity renames");
    assert_eq!(renamed.user_id, created.identity.user_id);
    assert!(matches!(
        storage.rename_identity(renamed.user_id, "straße").await,
        Err(StorageError::UsernameTaken)
    ));
    assert_eq!(
        storage
            .resolve_identity(&created.session_token)
            .await
            .expect("identity resolves")
            .expect("identity exists")
            .identity
            .username,
        "Bob"
    );
    storage.close().await;

    let reopened = Storage::open(&path).await.expect("storage reopens");
    assert_eq!(
        reopened
            .resolve_identity(&created.session_token)
            .await
            .expect("renamed identity resolves")
            .map(|session| session.identity),
        Some(renamed)
    );
    assert_eq!(
        reopened
            .resolve_identity(&"0".repeat(64))
            .await
            .expect("stale lookup succeeds"),
        None
    );
}
