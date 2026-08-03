use std::{fs, path::Path, time::UNIX_EPOCH};

use sqlx::{Connection, SqliteConnection, migrate::Migrator, sqlite::SqliteConnectOptions};
use tempfile::TempDir;

use filer_task_web::storage::{RedeemOutcome, Storage};

#[tokio::test]
async fn sessions_are_many_per_user_and_resolve_alongside_identity() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let first = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let second = storage
        .mint_recovery_session("Alice")
        .await
        .expect("recovery session mints");

    let first_resolved = storage
        .resolve_identity(&first.session_token)
        .await
        .expect("first session resolves")
        .expect("first session exists");
    let second_resolved = storage
        .resolve_identity(&second.session_token)
        .await
        .expect("second session resolves")
        .expect("second session exists");
    assert_eq!(
        first_resolved.identity.user_id,
        second_resolved.identity.user_id
    );
    assert_eq!(first_resolved.identity.username, "Alice");
    assert_ne!(first_resolved.session_id, second_resolved.session_id);
    storage.close().await;
}

#[tokio::test]
async fn resolve_identity_advances_last_seen_at_most_once_every_five_minutes() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");

    set_last_seen(&db, &created.session_token, unix_now() - 600).await;
    storage
        .resolve_identity(&created.session_token)
        .await
        .expect("stale resolve succeeds");
    let advanced = last_seen(&db, &created.session_token).await;
    assert!(
        unix_now() - advanced <= 1,
        "stale resolve advanced last_seen to {advanced}"
    );

    storage
        .resolve_identity(&created.session_token)
        .await
        .expect("fresh resolve succeeds");
    assert_eq!(
        last_seen(&db, &created.session_token).await,
        advanced,
        "fresh resolve does not rewrite last_seen"
    );
}

#[tokio::test]
async fn renaming_a_user_is_seen_by_every_session() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let first = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let second = storage
        .mint_recovery_session("Alice")
        .await
        .expect("recovery session mints");

    let renamed = storage
        .rename_identity(first.identity.user_id, "Bob")
        .await
        .expect("identity renames");
    assert_eq!(renamed.user_id, first.identity.user_id);
    for token in [&first.session_token, &second.session_token] {
        let resolved = storage
            .resolve_identity(token)
            .await
            .expect("session resolves")
            .expect("session exists");
        assert_eq!(resolved.identity.username, "Bob");
    }
    storage.close().await;
}

#[tokio::test]
async fn pre_migration_identity_cookie_becomes_a_session_row() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    stage_schema_four(&temp, &db).await;

    let storage = Storage::open(&db).await.expect("storage migrates");
    assert_eq!(storage.schema_version().await.expect("version reads"), 5);

    let cookie = "a".repeat(64);
    let resolved = storage
        .resolve_identity(&cookie)
        .await
        .expect("old cookie lookup succeeds")
        .expect("old cookie keeps working");
    assert_eq!(resolved.identity.username, "Alice");

    let mut connection = sqlite_connection(&db).await;
    let session_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&mut connection)
        .await
        .expect("session count reads");
    assert_eq!(session_rows, 1);
    let token_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'session_token'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("column check reads");
    assert_eq!(token_columns, 0, "users no longer owns a session token");
    connection.close().await.expect("connection closes");
    storage.close().await;
}

#[tokio::test]
async fn pairing_pin_mints_six_digits_and_the_newest_mint_replaces_the_live_one() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&created.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;

    let first = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");
    assert_eq!(first.pin.len(), 6);
    assert!(first.pin.chars().all(|digit| digit.is_ascii_digit()));
    assert!(first.expires_at > unix_now());

    let second = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("replacement pin mints");
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &first.pin).await,
        Ok(RedeemOutcome::WrongPin)
    ));
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &second.pin).await,
        Ok(RedeemOutcome::Session(_))
    ));
    storage.close().await;
}

#[tokio::test]
async fn expired_pairing_pin_is_rejected() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&created.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;
    let pin = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");

    let mut connection = sqlite_connection(&db).await;
    sqlx::query("UPDATE pairing_pins SET expires_at = unixepoch() - 1 WHERE pin = ?")
        .bind(&pin.pin)
        .execute(&mut connection)
        .await
        .expect("pin expires");
    connection.close().await.expect("connection closes");

    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &pin.pin).await,
        Ok(RedeemOutcome::Expired)
    ));
    storage.close().await;
}

#[tokio::test]
async fn pairing_pin_is_consumed_by_its_first_successful_redemption() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&created.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;
    let pin = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");

    match storage.redeem_pairing_pin("Alice", &pin.pin).await {
        Ok(RedeemOutcome::Session(session)) => {
            assert_eq!(session.identity.user_id, created.identity.user_id);
            assert_eq!(session.session_token.len(), 64);
            assert_ne!(session.session_token, created.session_token);
        }
        other => panic!("first redemption must succeed, got {other:?}"),
    }
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &pin.pin).await,
        Ok(RedeemOutcome::Consumed)
    ));
    storage.close().await;
}

#[tokio::test]
async fn wrong_pairing_pins_are_rejected_with_a_distinct_outcome() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let alice = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    storage
        .create_identity("Bob")
        .await
        .expect("identity creates");
    let alice_session = storage
        .resolve_identity(&alice.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;
    let pin = storage
        .mint_pairing_pin(alice.identity.user_id, alice_session)
        .await
        .expect("pin mints");

    let unknown = if pin.pin == "000000" {
        "000001"
    } else {
        "000000"
    };
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", unknown).await,
        Ok(RedeemOutcome::WrongPin)
    ));
    assert!(matches!(
        storage.redeem_pairing_pin("Bob", &pin.pin).await,
        Ok(RedeemOutcome::WrongPin)
    ));
    assert!(matches!(
        storage.redeem_pairing_pin("Carol", &pin.pin).await,
        Ok(RedeemOutcome::UnknownUsername)
    ));
    storage.close().await;
}

#[tokio::test]
async fn pairing_pin_is_invalidated_after_five_failed_attempts_and_re_minting_resets_it() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let alice = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    storage
        .create_identity("Bob")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&alice.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;
    let pin = storage
        .mint_pairing_pin(alice.identity.user_id, session_id)
        .await
        .expect("pin mints");

    for _ in 0..5 {
        assert!(matches!(
            storage.redeem_pairing_pin("Bob", &pin.pin).await,
            Ok(RedeemOutcome::WrongPin)
        ));
    }
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &pin.pin).await,
        Ok(RedeemOutcome::Locked)
    ));

    let fresh = storage
        .mint_pairing_pin(alice.identity.user_id, session_id)
        .await
        .expect("replacement pin mints");
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &fresh.pin).await,
        Ok(RedeemOutcome::Session(_))
    ));
    storage.close().await;
}

#[tokio::test]
async fn recovery_mint_creates_a_missing_user_and_adds_sessions_to_an_existing_one() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let first = storage
        .mint_recovery_session("Carol")
        .await
        .expect("recovery mint creates the user");
    assert_eq!(first.identity.username, "Carol");
    assert_eq!(first.session_token.len(), 64);

    let second = storage
        .mint_recovery_session("Carol")
        .await
        .expect("recovery mint adds a session");
    let first_resolved = storage
        .resolve_identity(&first.session_token)
        .await
        .expect("first session resolves")
        .expect("first session exists");
    let second_resolved = storage
        .resolve_identity(&second.session_token)
        .await
        .expect("second session resolves")
        .expect("second session exists");
    assert_eq!(
        first_resolved.identity.user_id,
        second_resolved.identity.user_id
    );
    assert_ne!(first_resolved.session_id, second_resolved.session_id);
    storage.close().await;
}

#[tokio::test]
async fn clearing_a_users_sessions_revokes_every_cookie_and_missing_users_clear_to_zero() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    storage
        .mint_recovery_session("Alice")
        .await
        .expect("second session mints");

    assert_eq!(
        storage
            .clear_user_sessions("Alice")
            .await
            .expect("sessions clear"),
        2
    );
    assert!(
        storage
            .resolve_identity(&created.session_token)
            .await
            .expect("revoked session lookup succeeds")
            .is_none()
    );
    assert_eq!(
        storage
            .clear_user_sessions("Nobody")
            .await
            .expect("missing user clears to zero"),
        0
    );
    let recovery = storage
        .mint_recovery_session("Alice")
        .await
        .expect("the identity can still gain a fresh session");
    assert_eq!(recovery.identity.user_id, created.identity.user_id);
    storage.close().await;
}

#[tokio::test]
async fn concurrent_redemptions_of_the_same_pin_yield_exactly_one_session() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&created.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;
    let pin = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");

    let (first, second) = tokio::join!(
        storage.redeem_pairing_pin("Alice", &pin.pin),
        storage.redeem_pairing_pin("Alice", &pin.pin),
    );
    let outcomes = [first, second];
    let sessions = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Ok(RedeemOutcome::Session(_))))
        .count();
    let consumed = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Ok(RedeemOutcome::Consumed)))
        .count();
    assert_eq!(sessions, 1, "exactly one redemption wins the code");
    assert_eq!(consumed, 1, "the loser sees the code as consumed");
    storage.close().await;
}

#[tokio::test]
async fn guessing_nonexistent_pins_locks_the_username_and_a_correct_pin_resets_it() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&created.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;
    let pin = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");

    for _ in 0..5 {
        assert!(matches!(
            storage.redeem_pairing_pin("Alice", "000000").await,
            Ok(RedeemOutcome::WrongPin)
        ));
    }
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", "111111").await,
        Ok(RedeemOutcome::Locked)
    ));

    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &pin.pin).await,
        Ok(RedeemOutcome::Session(_))
    ));
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", "222222").await,
        Ok(RedeemOutcome::WrongPin)
    ));
    storage.close().await;
}

#[tokio::test]
async fn minting_purges_consumed_and_expired_pins() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let created = storage
        .create_identity("Alice")
        .await
        .expect("identity creates");
    let session_id = storage
        .resolve_identity(&created.session_token)
        .await
        .expect("session resolves")
        .expect("session exists")
        .session_id;

    let consumed = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");
    assert!(matches!(
        storage.redeem_pairing_pin("Alice", &consumed.pin).await,
        Ok(RedeemOutcome::Session(_))
    ));

    let expired = storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("pin mints");
    let mut connection = sqlite_connection(&db).await;
    sqlx::query("UPDATE pairing_pins SET expires_at = unixepoch() - 1 WHERE pin = ?")
        .bind(&expired.pin)
        .execute(&mut connection)
        .await
        .expect("pin expires");
    connection.close().await.expect("connection closes");

    storage
        .mint_pairing_pin(created.identity.user_id, session_id)
        .await
        .expect("replacement pin mints");
    let mut connection = sqlite_connection(&db).await;
    let dead_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pairing_pins WHERE pin = ? OR pin = ?")
            .bind(&consumed.pin)
            .bind(&expired.pin)
            .fetch_one(&mut connection)
            .await
            .expect("dead pin count reads");
    connection.close().await.expect("connection closes");
    assert_eq!(
        dead_rows, 0,
        "consumed and expired pins are purged on the next mint"
    );
    storage.close().await;
}

async fn stage_schema_four(temp: &TempDir, db: &Path) {
    let staged = temp.path().join("staged-migrations");
    fs::create_dir_all(&staged).expect("staged migrations dir creates");
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "0001_storage_foundation.sql",
        "0002_project_registrations.sql",
        "0003_users.sql",
        "0004_activity.sql",
    ] {
        fs::copy(crate_dir.join("migrations").join(name), staged.join(name))
            .expect("migration file copies");
    }
    let migrator = Migrator::new(staged).await.expect("staged migrator loads");
    let mut connection = sqlite_connection(db).await;
    migrator
        .run(&mut connection)
        .await
        .expect("schema four applies");
    sqlx::query(
        "INSERT INTO users (session_token, display_name, name_key) \
         VALUES (?, 'Alice', 'alice')",
    )
    .bind("a".repeat(64))
    .execute(&mut connection)
    .await
    .expect("legacy identity inserts");
    connection.close().await.expect("connection closes");
}

async fn set_last_seen(db: &Path, token: &str, value: i64) {
    let mut connection = sqlite_connection(db).await;
    sqlx::query("UPDATE sessions SET last_seen = ? WHERE token = ?")
        .bind(value)
        .bind(token)
        .execute(&mut connection)
        .await
        .expect("last_seen updates");
    connection.close().await.expect("connection closes");
}

async fn last_seen(db: &Path, token: &str) -> i64 {
    let mut connection = sqlite_connection(db).await;
    let value = sqlx::query_scalar("SELECT last_seen FROM sessions WHERE token = ?")
        .bind(token)
        .fetch_one(&mut connection)
        .await
        .expect("last_seen reads");
    connection.close().await.expect("connection closes");
    value
}

async fn sqlite_connection(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection opens")
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs() as i64
}
