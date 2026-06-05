use std::{fs, process::Command};

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn validate_command_accepts_minimal_task_repo() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-001-location-routing.md",
        "CORE-001",
        "Location routing",
        "To Do",
        "High",
        "",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["validate", "--root"])
        .arg(repo.path())
        .output()
        .expect("validate command should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(stdout.contains("task validation passed (1 task(s))"));
}

#[test]
fn list_command_emits_parseable_json() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-001-location-routing.md",
        "CORE-001",
        "Location routing",
        "In Progress",
        "High",
        "tags: [location]\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "list",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
            "--status",
            "In Progress",
            "--format",
            "json",
        ])
        .output()
        .expect("list command should run");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        json.as_array()
            .expect("JSON output should be an array")
            .len(),
        1
    );
    assert_eq!(json[0]["id"], "CORE-001");
}

fn task_repo() -> TempDir {
    let temp = tempfile::tempdir().expect("temp dir should be created");
    fs::create_dir_all(temp.path().join(".tasks/core")).expect("core task dir should exist");
    fs::create_dir_all(temp.path().join(".tasks/app")).expect("app task dir should exist");
    fs::create_dir_all(temp.path().join(".tasks/ecosystem"))
        .expect("ecosystem task dir should exist");
    fs::write(temp.path().join(".tasks/task.schema.json"), "{}").expect("schema should exist");
    temp
}

fn write_task(
    repo: &TempDir,
    relative: &str,
    id: &str,
    title: &str,
    status: &str,
    priority: &str,
    extra: &str,
) {
    fs::write(
        repo.path().join(".tasks").join(relative),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\n{extra}---\n"
        ),
    )
    .expect("task should be written");
}
