use filer_task_web::storage::{ProjectRegistration, Role, Storage, StorageError};
use tempfile::TempDir;

#[tokio::test]
async fn registering_inserts_the_owner_in_the_same_transaction() {
    let (storage, temp) = fresh_storage().await;
    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("identity creates")
        .identity
        .user_id;

    storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("alpha", temp.path().join("alpha")),
            alice,
        )
        .await
        .expect("registration commits");
    assert_eq!(
        storage
            .project_role("alpha", alice)
            .await
            .expect("role reads"),
        Some(Role::Owner)
    );

    let error = storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("beta", temp.path().join("beta")),
            9_999,
        )
        .await
        .expect_err("an unknown owner is refused");
    assert!(matches!(error, StorageError::Operation { .. }), "{error}");
    let names: Vec<_> = storage
        .project_registrations()
        .await
        .expect("registrations load")
        .into_iter()
        .map(|registration| registration.name)
        .collect();
    assert_eq!(
        names,
        vec!["alpha"],
        "the failed owner insert rolled back beta"
    );
    storage.close().await;
}

async fn fresh_storage() -> (Storage, TempDir) {
    let temp = tempfile::tempdir().expect("temp dir creates");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    (storage, temp)
}
