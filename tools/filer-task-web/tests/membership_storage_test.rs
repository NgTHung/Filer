//! # Membership Storage
//!
//! Exercises Storage::open and its persisted schema because backfill must preserve access across upgrades.

use std::{fs, path::Path};

use sqlx::{Connection, SqliteConnection, migrate::Migrator, sqlite::SqliteConnectOptions};
use tempfile::TempDir;

use filer_task_web::storage::Storage;

#[tokio::test]
async fn migration_backfills_the_admin_owners_and_editors() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    stage_schema_six(&temp, &db).await;
    let mut connection = sqlite_connection(&db).await;
    for (name, key) in [("Alice", "alice"), ("Bob", "bob"), ("Cara", "cara")] {
        sqlx::query("INSERT INTO users (display_name, name_key) VALUES (?, ?)")
            .bind(name)
            .bind(key)
            .execute(&mut connection)
            .await
            .expect("user inserts");
    }
    for name in ["alpha", "beta", "gamma"] {
        sqlx::query("INSERT INTO project_registrations (name, root) VALUES (?, ?)")
            .bind(name)
            .bind(name.as_bytes())
            .execute(&mut connection)
            .await
            .expect("registration inserts");
    }
    // Stale activity must not replace a valid owner or resurrect removed users and projects.
    for (user_id, username, project, action) in [
        (1, "Alice", "alpha", "project.register"),
        (3, "Cara", "alpha", "task.create"),
        (2, "Bob", "alpha", "project.register"),
        (2, "Bob", "beta", "task.edit"),
        (3, "Cara", "ghost", "project.register"),
        (99, "Deleted", "alpha", "project.register"),
        (99, "Deleted", "beta", "task.edit"),
        (99, "Deleted", "gamma", "project.register"),
    ] {
        sqlx::query(
            "INSERT INTO activity (user_id, username, project, action) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(username)
        .bind(project)
        .bind(action)
        .execute(&mut connection)
        .await
        .expect("activity inserts");
    }
    connection.close().await.expect("connection closes");

    let storage = Storage::open(&db).await.expect("storage migrates");
    assert_eq!(storage.schema_version().await.expect("version reads"), 7);
    storage.close().await;

    assert_eq!(
        admin_flags(&db).await,
        vec![
            ("Alice".to_string(), true),
            ("Bob".to_string(), false),
            ("Cara".to_string(), false),
        ]
    );
    assert_eq!(
        member_rows(&db, "alpha").await,
        vec![
            ("Bob".to_string(), "owner".to_string()),
            ("Alice".to_string(), "editor".to_string()),
            ("Cara".to_string(), "editor".to_string()),
        ]
    );
    assert_eq!(
        member_rows(&db, "beta").await,
        vec![
            ("Alice".to_string(), "owner".to_string()),
            ("Bob".to_string(), "editor".to_string()),
        ]
    );
    assert_eq!(
        member_rows(&db, "gamma").await,
        vec![("Alice".to_string(), "owner".to_string())]
    );
    assert!(member_rows(&db, "ghost").await.is_empty());
}

#[tokio::test]
async fn migration_leaves_projects_ownerless_without_users() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    stage_schema_six(&temp, &db).await;
    let mut connection = sqlite_connection(&db).await;
    sqlx::query("INSERT INTO project_registrations (name, root) VALUES ('alpha', X'61')")
        .execute(&mut connection)
        .await
        .expect("registration inserts");
    connection.close().await.expect("connection closes");

    let storage = Storage::open(&db).await.expect("storage migrates");
    storage.close().await;

    assert!(member_rows(&db, "alpha").await.is_empty());
}

async fn stage_schema_six(temp: &TempDir, db: &Path) {
    let staged = temp.path().join("staged-migrations");
    fs::create_dir_all(&staged).expect("staged migrations dir creates");
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "0001_storage_foundation.sql",
        "0002_project_registrations.sql",
        "0003_users.sql",
        "0004_activity.sql",
        "0005_sessions.sql",
        "0006_session_device_labels.sql",
    ] {
        fs::copy(crate_dir.join("migrations").join(name), staged.join(name))
            .expect("migration file copies");
    }
    let migrator = Migrator::new(staged).await.expect("staged migrator loads");
    let mut connection = sqlite_connection(db).await;
    migrator
        .run(&mut connection)
        .await
        .expect("schema six applies");
    connection.close().await.expect("connection closes");
}

async fn sqlite_connection(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection opens")
}

/// Members of `project` as (username, role), owners first, then by name.
async fn member_rows(db: &Path, project: &str) -> Vec<(String, String)> {
    let mut connection = sqlite_connection(db).await;
    let rows = sqlx::query_as(
        "SELECT u.display_name, m.role FROM project_members m \
         JOIN users u ON u.id = m.user_id WHERE m.project = ? \
         ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, u.name_key",
    )
    .bind(project)
    .fetch_all(&mut connection)
    .await
    .expect("members read");
    connection.close().await.expect("connection closes");
    rows
}

async fn admin_flags(db: &Path) -> Vec<(String, bool)> {
    let mut connection = sqlite_connection(db).await;
    let rows = sqlx::query_as("SELECT display_name, is_admin FROM users ORDER BY id")
        .fetch_all(&mut connection)
        .await
        .expect("admin flags read");
    connection.close().await.expect("connection closes");
    rows
}

#[tokio::test]
async fn membership_schema_rejects_invalid_roles_admin_flags_and_references() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let mut connection = sqlite_connection(&db).await;
    sqlx::query("INSERT INTO users (id, display_name, name_key) VALUES (1, 'Alice', 'alice')")
        .execute(&mut connection)
        .await
        .expect("user inserts");
    sqlx::query("INSERT INTO project_registrations (name, root) VALUES ('alpha', X'61')")
        .execute(&mut connection)
        .await
        .expect("project inserts");

    for statement in [
        "UPDATE users SET is_admin = 2 WHERE id = 1",
        "INSERT INTO project_members (project, user_id, role) VALUES ('alpha', 1, 'viewer')",
        "INSERT INTO project_members (project, user_id, role) VALUES ('ghost', 1, 'owner')",
        "INSERT INTO project_members (project, user_id, role) VALUES ('alpha', 99, 'owner')",
        "INSERT INTO project_members (project, user_id, role, granted_by) VALUES ('alpha', 1, 'owner', 99)",
        "INSERT INTO access_requests (project, user_id) VALUES ('ghost', 1)",
        "INSERT INTO access_requests (project, user_id) VALUES ('alpha', 99)",
    ] {
        let error = sqlx::query(statement)
            .execute(&mut connection)
            .await
            .expect_err("invalid row is rejected");
        assert!(
            matches!(error, sqlx::Error::Database(_)),
            "{statement}: {error}"
        );
    }
    connection.close().await.expect("connection closes");
    storage.close().await;
}

#[tokio::test]
async fn memberships_and_requests_are_unique_and_cascade_when_a_project_is_removed() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let mut connection = sqlite_connection(&db).await;
    sqlx::query("INSERT INTO users (id, display_name, name_key) VALUES (1, 'Alice', 'alice')")
        .execute(&mut connection)
        .await
        .expect("user inserts");
    for project in ["alpha", "beta"] {
        sqlx::query("INSERT INTO project_registrations (name, root) VALUES (?, ?)")
            .bind(project)
            .bind(project.as_bytes())
            .execute(&mut connection)
            .await
            .expect("project inserts");
        sqlx::query("INSERT INTO project_members (project, user_id, role) VALUES (?, 1, 'editor')")
            .bind(project)
            .execute(&mut connection)
            .await
            .expect("membership inserts");
        sqlx::query("INSERT INTO access_requests (project, user_id) VALUES (?, 1)")
            .bind(project)
            .execute(&mut connection)
            .await
            .expect("request inserts");
    }
    for statement in [
        "INSERT INTO project_members (project, user_id, role) VALUES ('alpha', 1, 'owner')",
        "INSERT INTO access_requests (project, user_id) VALUES ('alpha', 1)",
        "DELETE FROM users WHERE id = 1",
    ] {
        assert!(
            sqlx::query(statement)
                .execute(&mut connection)
                .await
                .is_err(),
            "duplicate rows and referenced-user deletion are rejected: {statement}"
        );
    }
    let timestamps: (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT m.granted_at, r.requested_at, m.granted_by FROM project_members m \
         JOIN access_requests r USING (project, user_id) WHERE m.project = 'alpha'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("grant and request defaults read");
    assert!(timestamps.0 > 0);
    assert!(timestamps.1 > 0);
    assert_eq!(timestamps.2, None);

    sqlx::query("DELETE FROM project_registrations WHERE name = 'alpha'")
        .execute(&mut connection)
        .await
        .expect("project removes");
    let requests: Vec<String> =
        sqlx::query_scalar("SELECT project FROM access_requests ORDER BY project")
            .fetch_all(&mut connection)
            .await
            .expect("requests read");
    assert_eq!(requests, vec!["beta"]);
    connection.close().await.expect("connection closes");
    assert!(member_rows(&db, "alpha").await.is_empty());
    assert_eq!(
        member_rows(&db, "beta").await,
        vec![("Alice".to_string(), "editor".to_string())]
    );
    storage.close().await;
}

#[tokio::test]
async fn the_first_identity_on_an_empty_database_becomes_admin() {
    let (storage, _temp) = fresh_storage().await;
    let alice = storage
        .create_identity("Alice", "Browser A")
        .await
        .expect("first identity creates");
    let bob = storage
        .create_identity("Bob", "Browser B")
        .await
        .expect("second identity creates");
    assert!(
        storage
            .is_admin(alice.identity.user_id)
            .await
            .expect("flag reads")
    );
    assert!(
        !storage
            .is_admin(bob.identity.user_id)
            .await
            .expect("flag reads")
    );
    assert!(!storage.is_admin(99).await.expect("missing flag reads"));
    for (session, expected) in [(&alice, true), (&bob, false)] {
        let resolved = storage
            .resolve_identity(&session.session_token)
            .await
            .expect("lookup succeeds")
            .expect("session exists");
        assert_eq!(resolved.is_admin, expected);
    }
    assert_eq!(storage.admin_count().await.expect("admins count"), 1);
    storage.close().await;
}

async fn fresh_storage() -> (Storage, TempDir) {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    (storage, temp)
}

#[tokio::test]
async fn concurrent_first_identities_produce_exactly_one_admin() {
    let (storage, _temp) = fresh_storage().await;
    let (first, second) = tokio::join!(
        storage.create_identity("Alice", "Browser A"),
        storage.create_identity("Bob", "Browser B"),
    );
    first.expect("first identity creates");
    second.expect("second identity creates");
    assert_eq!(storage.admin_count().await.expect("admins count"), 1);
    storage.close().await;
}

#[tokio::test]
async fn recovery_creates_the_first_admin_and_preserves_existing_admin_flags() {
    let (storage, _temp) = fresh_storage().await;
    let alice = storage
        .mint_recovery_session("Alice", "Recovery CLI")
        .await
        .expect("first recovery mints");
    let bob = storage
        .mint_recovery_session("Bob", "Recovery CLI")
        .await
        .expect("second recovery mints");
    let recovered = storage
        .mint_recovery_session("Alice", "Recovery CLI")
        .await
        .expect("existing recovery mints");
    assert_eq!(recovered.identity.user_id, alice.identity.user_id);
    assert!(
        storage
            .is_admin(alice.identity.user_id)
            .await
            .expect("flag reads")
    );
    assert!(
        !storage
            .is_admin(bob.identity.user_id)
            .await
            .expect("flag reads")
    );
    assert_eq!(storage.admin_count().await.expect("admins count"), 1);
    storage.close().await;
}

#[tokio::test]
async fn concurrent_browser_and_recovery_creation_produce_one_admin() {
    let (storage, _temp) = fresh_storage().await;
    let (browser, recovery) = tokio::join!(
        storage.create_identity("Alice", "Browser"),
        storage.mint_recovery_session("Bob", "Recovery CLI"),
    );
    browser.expect("browser identity creates");
    recovery.expect("recovery identity creates");
    assert_eq!(storage.admin_count().await.expect("admins count"), 1);
    storage.close().await;
}
