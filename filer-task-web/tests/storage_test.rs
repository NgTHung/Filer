use std::path::Path;

use filer_task_web::storage::{Storage, StorageError};
use sqlx::{Connection, Executor, SqliteConnection, sqlite::SqliteConnectOptions};

#[tokio::test]
async fn nonexistent_path_creates_database_at_schema_version_one() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let path = temp.path().join("state.sqlite3");

    let storage = Storage::open(&path).await.expect("storage opens");

    assert!(path.is_file());
    assert_eq!(storage.schema_version().await.expect("version reads"), 1);
    storage.close().await;
}

#[tokio::test]
async fn reopening_migrated_database_is_idempotent() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let path = temp.path().join("state.sqlite3");
    let storage = Storage::open(&path).await.expect("storage opens");
    storage.close().await;

    let reopened = Storage::open(&path).await.expect("storage reopens");

    assert_eq!(reopened.schema_version().await.expect("version reads"), 1);
    reopened.close().await;
}

#[tokio::test]
async fn preexisting_version_zero_database_migrates_to_version_one() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let path = temp.path().join("state.sqlite3");
    let mut connection = sqlite_connection(&path).await;
    connection
        .execute("PRAGMA user_version = 0")
        .await
        .expect("version zero database created");
    connection.close().await.expect("connection closes");

    let storage = Storage::open(&path).await.expect("storage opens");

    assert_eq!(storage.schema_version().await.expect("version reads"), 1);
    storage.close().await;
}

#[tokio::test]
async fn missing_parent_directory_returns_open_error_with_path() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let path = temp.path().join("missing").join("state.sqlite3");

    let error = Storage::open(&path)
        .await
        .expect_err("missing parent must fail");

    assert!(matches!(
        &error,
        StorageError::Open { path: error_path, .. } if error_path == &path
    ));
    assert!(error.to_string().contains(&path.display().to_string()));
}

#[tokio::test]
async fn incompatible_migration_checksum_returns_migrate_error() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let path = temp.path().join("state.sqlite3");
    let storage = Storage::open(&path).await.expect("storage opens");
    storage.close().await;
    let mut connection = sqlite_connection(&path).await;
    connection
        .execute("UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 1")
        .await
        .expect("checksum corrupted");
    connection.close().await.expect("connection closes");

    let error = Storage::open(&path)
        .await
        .expect_err("checksum mismatch must fail");

    assert!(matches!(
        &error,
        StorageError::Migrate { path: error_path, .. } if error_path == &path
    ));
    assert!(error.to_string().contains(&path.display().to_string()));
}

async fn sqlite_connection(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection opens")
}
