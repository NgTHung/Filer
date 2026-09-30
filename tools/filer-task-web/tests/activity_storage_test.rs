use filer_task_web::storage::{ActivityFilter, NewActivity, Storage};
use tempfile::TempDir;

// The caller keeps the directory alive, because each new pooled connection opens the database path again.
async fn open_storage() -> (Storage, TempDir) {
    let database = tempfile::tempdir().expect("database directory created");
    let storage = Storage::open(database.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    (storage, database)
}

async fn record(
    storage: &Storage,
    user_id: i64,
    project: &str,
    task_id: Option<&str>,
    action: &str,
) {
    storage
        .record_activity(NewActivity {
            user_id,
            username: "Ada Lovelace",
            project,
            task_id,
            action,
            detail: None,
        })
        .await
        .expect("activity records");
}

#[tokio::test]
async fn recording_a_committed_write_absorbs_a_storage_failure() {
    let (storage, _database) = open_storage().await;
    storage.close().await;

    storage
        .record_committed_activity(NewActivity {
            user_id: 1,
            username: "Ada Lovelace",
            project: "alpha",
            task_id: Some("core:CORE-001"),
            action: "task.create",
            detail: None,
        })
        .await;

    assert!(
        storage
            .list_activity(ActivityFilter::default())
            .await
            .is_err(),
        "the pool stays closed, so the call absorbed a real failure"
    );
}

#[tokio::test]
async fn list_activity_returns_newest_first() {
    let (storage, _database) = open_storage().await;
    record(&storage, 1, "alpha", Some("core:CORE-001"), "task.create").await;
    record(&storage, 1, "alpha", Some("core:CORE-002"), "task.create").await;
    record(&storage, 1, "alpha", Some("core:CORE-003"), "task.create").await;

    let rows = storage
        .list_activity(ActivityFilter {
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("activity lists");

    let ids: Vec<_> = rows.iter().map(|row| row.task_id.clone()).collect();
    assert_eq!(
        ids,
        vec![
            Some("core:CORE-003".to_string()),
            Some("core:CORE-002".to_string()),
            Some("core:CORE-001".to_string()),
        ]
    );
}

#[tokio::test]
async fn list_activity_filters_by_project() {
    let (storage, _database) = open_storage().await;
    record(&storage, 1, "alpha", Some("core:CORE-001"), "task.create").await;
    record(&storage, 1, "beta", Some("web:WEB-001"), "task.create").await;

    let rows = storage
        .list_activity(ActivityFilter {
            project: Some("beta".to_string()),
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("activity lists");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].project, "beta");
}

#[tokio::test]
async fn list_activity_filters_by_task_id() {
    let (storage, _database) = open_storage().await;
    record(&storage, 1, "alpha", Some("core:CORE-001"), "task.create").await;
    record(&storage, 1, "alpha", Some("core:CORE-001"), "task.done").await;
    record(&storage, 1, "alpha", Some("core:CORE-002"), "task.create").await;

    let rows = storage
        .list_activity(ActivityFilter {
            task_id: Some("core:CORE-001".to_string()),
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("activity lists");

    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| row.task_id.as_deref() == Some("core:CORE-001"))
    );
}

#[tokio::test]
async fn list_activity_paginates_with_limit_and_offset() {
    let (storage, _database) = open_storage().await;
    for index in 0..5 {
        record(
            &storage,
            1,
            "alpha",
            Some(&format!("core:CORE-{index:03}")),
            "task.create",
        )
        .await;
    }

    let page = storage
        .list_activity(ActivityFilter {
            limit: 2,
            offset: 1,
            ..Default::default()
        })
        .await
        .expect("activity lists");

    let ids: Vec<_> = page.iter().map(|row| row.task_id.clone()).collect();
    assert_eq!(
        ids,
        vec![
            Some("core:CORE-003".to_string()),
            Some("core:CORE-002".to_string()),
        ]
    );
}

#[tokio::test]
async fn recorded_activity_captures_actor_and_action() {
    let (storage, _database) = open_storage().await;
    storage
        .record_activity(NewActivity {
            user_id: 7,
            username: "Grace Hopper",
            project: "alpha",
            task_id: Some("core:CORE-001"),
            action: "task.block",
            detail: Some("waiting on review"),
        })
        .await
        .expect("activity records");

    let rows = storage
        .list_activity(ActivityFilter {
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .expect("activity lists");

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.username, "Grace Hopper");
    assert_eq!(row.project, "alpha");
    assert_eq!(row.task_id.as_deref(), Some("core:CORE-001"));
    assert_eq!(row.action, "task.block");
    assert_eq!(row.detail.as_deref(), Some("waiting on review"));
    assert!(row.created_at > 0);
}
