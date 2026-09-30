//! Exercises the recovery subcommands of the `filer-task-web` binary end to
//! end: `session-mint` prints a cookie value that resolves to the requested
//! user, and `session-clear` revokes every cookie of a user and prints how many
//! it removed. Each test runs the real binary against its own temp database.

use std::process::{Command, Output};

use filer_task_web::storage::Storage;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_filer-task-web")
}

fn run(args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("CLI process spawns")
}

fn run_ok(args: &[&str]) -> Output {
    let output = run(args);
    assert!(
        output.status.success(),
        "expected success, got {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn stdout_trimmed(output: &Output) -> String {
    String::from_utf8(output.stdout.clone())
        .expect("CLI stdout is UTF-8")
        .trim()
        .to_string()
}

#[tokio::test]
async fn session_mint_prints_a_resolvable_session_cookie() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let db_arg = db.display().to_string();

    let first = run_ok(&["session-mint", "Alice", "--database", &db_arg]);
    let first_token = stdout_trimmed(&first);
    assert_eq!(first_token.len(), 64);

    let storage = Storage::open(&db).await.expect("storage opens");
    let resolved = storage
        .resolve_identity(&first_token)
        .await
        .expect("lookup succeeds")
        .expect("session exists");
    assert_eq!(resolved.identity.username, "Alice");

    let second = run_ok(&["session-mint", "Alice", "--database", &db_arg]);
    let second_token = stdout_trimmed(&second);
    assert_ne!(first_token, second_token);
    let second_resolved = storage
        .resolve_identity(&second_token)
        .await
        .expect("lookup succeeds")
        .expect("session exists");
    assert_eq!(second_resolved.identity.user_id, resolved.identity.user_id);
    storage.close().await;
}

#[tokio::test]
async fn session_mint_creates_a_missing_user() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let db_arg = db.display().to_string();

    let minted = run_ok(&["session-mint", "Bob", "--database", &db_arg]);
    let token = stdout_trimmed(&minted);
    assert_eq!(token.len(), 64);

    let storage = Storage::open(&db).await.expect("storage opens");
    let resolved = storage
        .resolve_identity(&token)
        .await
        .expect("lookup succeeds")
        .expect("session exists");
    assert_eq!(resolved.identity.username, "Bob");
    storage.close().await;
}

#[tokio::test]
async fn session_clear_revokes_every_session_and_reports_the_count() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let db_arg = db.display().to_string();

    let first = run_ok(&["session-mint", "Alice", "--database", &db_arg]);
    let first_token = stdout_trimmed(&first);
    let second = run_ok(&["session-mint", "Alice", "--database", &db_arg]);
    let second_token = stdout_trimmed(&second);

    let cleared = run_ok(&["session-clear", "Alice", "--database", &db_arg]);
    assert_eq!(stdout_trimmed(&cleared), "2");

    let storage = Storage::open(&db).await.expect("storage opens");
    assert!(
        storage
            .resolve_identity(&first_token)
            .await
            .expect("lookup succeeds")
            .is_none()
    );
    assert!(
        storage
            .resolve_identity(&second_token)
            .await
            .expect("lookup succeeds")
            .is_none()
    );

    let none = run_ok(&["session-clear", "Nobody", "--database", &db_arg]);
    assert_eq!(stdout_trimmed(&none), "0");
    storage.close().await;
}

#[test]
fn recovery_commands_reject_bad_usage() {
    let cases: &[&[&str]] = &[
        &["session-mint"],
        &["session-mint", "Alice", "extra"],
        &["session-mint", "Alice", "--databse", "x"],
        &["session-clear"],
        &["frobnicate"],
        &["--databse", "x"],
    ];
    for case in cases {
        let output = run(case);
        assert!(
            !output.status.success(),
            "expected failure for {case:?}\nstdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            !output.stderr.is_empty(),
            "expected a usage message on stderr for {case:?}"
        );
    }
}
