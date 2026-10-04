//! Exercises the membership subcommands of the `filer-task-web` binary end to
//! end against a temp database, and the startup warning for exposed hosts.

mod cli;

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use cli::run;
use filer_task_web::{
    access::exposure_warning,
    storage::{ProjectRegistration, Role, Storage},
};
use tempfile::TempDir;

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

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Alice (admin, owner of alpha) and Bob (no role) in a fresh database.
async fn seeded() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("alice creates")
        .identity
        .user_id;
    storage
        .create_identity("Bob", "Test browser")
        .await
        .expect("bob creates");
    storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("alpha", temp.path().join("alpha")),
            alice,
        )
        .await
        .expect("alpha registers");
    storage.close().await;
    (temp, db)
}

async fn open(db: &Path) -> Storage {
    Storage::open(db).await.expect("storage opens")
}

#[tokio::test]
async fn admin_grant_and_revoke_flip_the_flag() {
    let (_temp, db) = seeded().await;
    let db_arg = db.display().to_string();

    let granted = run_ok(&["admin-grant", "bob", "--database", &db_arg]);
    assert_eq!(stdout(&granted).trim(), "granted admin to Bob");
    let storage = open(&db).await;
    let bob = storage
        .user_by_name("Bob")
        .await
        .expect("lookup")
        .expect("bob exists");
    assert!(storage.is_admin(bob.user_id).await.expect("flag reads"));
    storage.close().await;

    run_ok(&["admin-revoke", "Bob", "--database", &db_arg]);
    let last = run_ok(&["admin-revoke", "Alice", "--database", &db_arg]);
    assert!(
        stderr(&last).contains("no admins remain"),
        "{}",
        stderr(&last)
    );

    let missing = run(&["admin-grant", "Nobody", "--database", &db_arg]);
    assert!(!missing.status.success());
    assert!(
        stderr(&missing).contains("no user named \"Nobody\""),
        "{}",
        stderr(&missing)
    );
}

#[tokio::test]
async fn member_commands_grant_list_and_revoke() {
    let (_temp, db) = seeded().await;
    let db_arg = db.display().to_string();
    let storage = open(&db).await;
    let bob = storage
        .user_by_name("Bob")
        .await
        .expect("lookup")
        .expect("bob exists");
    storage
        .request_access("alpha", bob.user_id)
        .await
        .expect("request records");
    storage.close().await;

    let granted = run_ok(&[
        "member-grant",
        "alpha",
        "Bob",
        "--role",
        "editor",
        "--database",
        &db_arg,
    ]);
    assert_eq!(stdout(&granted).trim(), "Bob is editor of alpha");
    let listed = run_ok(&["member-list", "alpha", "--database", &db_arg]);
    assert_eq!(stdout(&listed), "owner\tAlice\neditor\tBob\n");
    let storage = open(&db).await;
    assert!(
        storage
            .list_access_requests("alpha")
            .await
            .expect("requests list")
            .is_empty()
    );
    storage.close().await;

    let revoked = run_ok(&["member-revoke", "alpha", "Alice", "--database", &db_arg]);
    assert!(
        stderr(&revoked).contains("alpha has no owner"),
        "{}",
        stderr(&revoked)
    );
    let listed = run_ok(&["member-list", "alpha", "--database", &db_arg]);
    assert_eq!(stdout(&listed), "editor\tBob\n");

    let storage = open(&db).await;
    assert_eq!(
        storage
            .project_role("alpha", bob.user_id)
            .await
            .expect("role reads"),
        Some(Role::Editor)
    );
    storage.close().await;

    // as_str, not &db_arg: these arrays have no expected element type, so a
    // &String would not coerce to &str.
    let db_arg = db_arg.as_str();
    for (args, message) in [
        (
            vec![
                "member-grant",
                "ghost",
                "Bob",
                "--role",
                "editor",
                "--database",
                db_arg,
            ],
            "project ghost is not registered",
        ),
        (
            vec![
                "member-grant",
                "alpha",
                "Bob",
                "--role",
                "admin",
                "--database",
                db_arg,
            ],
            "invalid --role value \"admin\"",
        ),
        (
            vec!["member-grant", "alpha", "Bob", "--database", db_arg],
            "--role is required",
        ),
        (vec!["member-list", "--database", db_arg], "missing project"),
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "expected failure for {args:?}");
        assert!(
            stderr(&output).contains(message),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn the_exposure_warning_names_only_non_loopback_hosts() {
    for host in ["0.0.0.0", "192.168.1.20", "::"] {
        let warning = exposure_warning(host.parse().expect("address parses"))
            .unwrap_or_else(|| panic!("{host} should warn"));
        assert!(warning.contains(host), "{warning}");
        assert!(
            warning.contains("writes need project membership"),
            "{warning}"
        );
    }
    for host in ["127.0.0.1", "::1"] {
        assert_eq!(
            exposure_warning(host.parse().expect("address parses")),
            None,
            "{host}"
        );
    }
}

#[test]
fn serving_on_the_loopback_default_prints_no_warning() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3").display().to_string();
    let mut child = Command::new(cli::binary())
        .args(["--port", "0", "--database", &db])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server process spawns");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout is piped"))
        .read_line(&mut line)
        .expect("stdout is readable");
    assert!(line.starts_with("filer-task-web listening on"), "{line}");
    child.kill().expect("server stops");
    child.wait().expect("server exits");
    let mut errors = String::new();
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut errors)
        .expect("stderr is readable");
    assert!(!errors.contains("warning"), "{errors}");
}
