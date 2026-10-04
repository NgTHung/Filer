use filer_task_web::storage::{
    GrantOutcome, ProjectAccess, ProjectRegistration, RemovedMember, Role, Storage, StorageError,
};
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

#[tokio::test]
async fn approving_a_request_grants_the_role_and_deletes_the_request() {
    let (storage, temp) = fresh_storage().await;
    let (alice, bob) = owner_and_user(&storage, &temp).await;

    assert!(
        storage
            .request_access("alpha", bob)
            .await
            .expect("request records")
    );
    assert!(
        !storage
            .request_access("alpha", bob)
            .await
            .expect("repeat succeeds")
    );
    assert_eq!(
        storage
            .user_project_access(bob)
            .await
            .expect("access reads")
            .get("alpha"),
        Some(&ProjectAccess::Pending)
    );
    assert_eq!(
        storage
            .list_access_requests("alpha")
            .await
            .expect("requests list")[0]
            .username,
        "Bob"
    );

    let outcome = storage
        .grant_member("alpha", bob, Role::Editor, Some(alice), true)
        .await
        .expect("grant succeeds");

    assert_eq!(
        outcome,
        GrantOutcome {
            username: "Bob".to_string(),
            previous: None
        }
    );
    assert!(
        storage
            .list_access_requests("alpha")
            .await
            .expect("requests list")
            .is_empty()
    );
    assert_eq!(
        storage
            .user_project_access(bob)
            .await
            .expect("access reads")
            .get("alpha"),
        Some(&ProjectAccess::Editor)
    );
    let members = storage.list_members("alpha").await.expect("members list");
    assert_eq!(members[1].username, "Bob");
    assert_eq!(members[1].granted_by.as_deref(), Some("Alice"));
    let error = storage
        .request_access("alpha", bob)
        .await
        .expect_err("a member cannot request access");
    assert!(matches!(error, StorageError::AlreadyMember), "{error}");
    storage.close().await;
}

#[tokio::test]
async fn the_last_owner_cannot_be_removed_or_demoted_except_by_override() {
    let (storage, temp) = fresh_storage().await;
    let (alice, bob) = owner_and_user(&storage, &temp).await;
    storage
        .grant_member("alpha", bob, Role::Editor, Some(alice), true)
        .await
        .expect("bob joins");

    let demote = storage
        .grant_member("alpha", alice, Role::Editor, None, true)
        .await
        .expect_err("the last owner cannot be demoted");
    assert!(matches!(demote, StorageError::LastOwner), "{demote}");
    let remove = storage
        .remove_member("alpha", alice, true)
        .await
        .expect_err("the last owner cannot be removed");
    assert!(matches!(remove, StorageError::LastOwner), "{remove}");
    assert_eq!(
        storage
            .project_role("alpha", alice)
            .await
            .expect("role reads"),
        Some(Role::Owner)
    );

    let promoted = storage
        .grant_member("alpha", bob, Role::Owner, Some(alice), true)
        .await
        .expect("bob is promoted");
    assert_eq!(promoted.previous, Some(Role::Editor));
    let demoted = storage
        .grant_member("alpha", alice, Role::Editor, Some(bob), true)
        .await
        .expect("alice steps down while bob owns");
    assert_eq!(demoted.previous, Some(Role::Owner));

    let refused = storage
        .remove_member("alpha", bob, true)
        .await
        .expect_err("bob is the last owner");
    assert!(matches!(refused, StorageError::LastOwner), "{refused}");
    let removed = storage
        .remove_member("alpha", bob, false)
        .await
        .expect("the override removes the last owner");
    assert_eq!(
        removed,
        Some(RemovedMember {
            username: "Bob".to_string(),
            role: Role::Owner
        })
    );
    assert!(
        storage
            .list_members("alpha")
            .await
            .expect("members list")
            .iter()
            .all(|member| member.role != Role::Owner)
    );
    assert_eq!(
        storage
            .remove_member("alpha", bob, true)
            .await
            .expect("a non-member removes"),
        None
    );
    storage.close().await;
}

#[tokio::test]
async fn two_owners_demoting_each_other_at_once_leave_one_owner() {
    let (storage, temp) = fresh_storage().await;
    let (alice, bob) = owner_and_user(&storage, &temp).await;
    storage
        .grant_member("alpha", bob, Role::Owner, Some(alice), true)
        .await
        .expect("bob is an owner");

    let (first, second) = tokio::join!(
        storage.grant_member("alpha", alice, Role::Editor, Some(bob), true),
        storage.grant_member("alpha", bob, Role::Editor, Some(alice), true),
    );

    assert_eq!(
        [first.is_ok(), second.is_ok()]
            .iter()
            .filter(|ok| **ok)
            .count(),
        1,
        "first: {first:?}, second: {second:?}"
    );
    let refused = if first.is_err() { first } else { second };
    assert!(
        matches!(refused, Err(StorageError::LastOwner)),
        "{refused:?}"
    );
    let owners = storage
        .list_members("alpha")
        .await
        .expect("members list")
        .into_iter()
        .filter(|member| member.role == Role::Owner)
        .count();
    assert_eq!(owners, 1);
    storage.close().await;
}

#[tokio::test]
async fn grants_name_the_missing_user_or_project() {
    let (storage, temp) = fresh_storage().await;
    let (alice, _bob) = owner_and_user(&storage, &temp).await;

    let missing_user = storage
        .grant_member("alpha", 9_999, Role::Editor, Some(alice), true)
        .await
        .expect_err("unknown user is refused");
    assert!(
        matches!(missing_user, StorageError::IdentityNotFound(9_999)),
        "{missing_user}"
    );
    let missing_project = storage
        .grant_member("ghost", alice, Role::Editor, None, true)
        .await
        .expect_err("unknown project is refused");
    assert!(
        matches!(&missing_project, StorageError::ProjectNotRegistered(name) if name == "ghost"),
        "{missing_project}"
    );
    storage.close().await;
}

#[tokio::test]
async fn removing_a_registration_deletes_its_members_and_requests() {
    let (storage, temp) = fresh_storage().await;
    let (_alice, bob) = owner_and_user(&storage, &temp).await;
    storage
        .request_access("alpha", bob)
        .await
        .expect("request records");

    assert!(
        storage
            .delete_project_registration("alpha")
            .await
            .expect("registration deletes")
    );

    assert!(
        storage
            .list_members("alpha")
            .await
            .expect("members list")
            .is_empty()
    );
    assert!(
        storage
            .list_access_requests("alpha")
            .await
            .expect("requests list")
            .is_empty()
    );
    assert!(
        storage
            .user_project_access(bob)
            .await
            .expect("access reads")
            .is_empty()
    );
    storage.close().await;
}

#[tokio::test]
async fn deleting_a_request_reports_who_asked() {
    let (storage, temp) = fresh_storage().await;
    let (_alice, bob) = owner_and_user(&storage, &temp).await;
    storage
        .request_access("alpha", bob)
        .await
        .expect("request records");

    assert_eq!(
        storage
            .delete_access_request("alpha", bob)
            .await
            .expect("request deletes"),
        Some("Bob".to_string())
    );
    assert_eq!(
        storage
            .delete_access_request("alpha", bob)
            .await
            .expect("repeat deletes"),
        None
    );
    storage.close().await;
}

/// Alice owns a registered project named alpha; Bob exists with no access.
async fn owner_and_user(storage: &Storage, temp: &TempDir) -> (i64, i64) {
    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("alice creates")
        .identity
        .user_id;
    let bob = storage
        .create_identity("Bob", "Test browser")
        .await
        .expect("bob creates")
        .identity
        .user_id;
    storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("alpha", temp.path().join("alpha")),
            alice,
        )
        .await
        .expect("alpha registers");
    (alice, bob)
}

async fn fresh_storage() -> (Storage, TempDir) {
    let temp = tempfile::tempdir().expect("temp dir creates");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    (storage, temp)
}
