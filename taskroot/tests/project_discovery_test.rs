use std::{fs, path::Path, process::Command};

use serde_json::Value;
use taskroot::{error::TaskError, repo::discover_project_root};
use tempfile::TempDir;

#[test]
fn discovers_tasks_directory_from_root_nested_paths_and_files() {
    let repo = task_repo("CORE-101", "First project task");
    let nested = repo.path().join("src/deep");
    fs::create_dir_all(&nested).expect("nested directory should exist");
    let file = nested.join("input.txt");
    fs::write(&file, "input").expect("nested file should exist");

    for start in [repo.path(), nested.as_path(), file.as_path()] {
        assert_eq!(
            discover_project_root(start).expect("project should be discovered"),
            repo.path()
        );
    }

    let nonexistent = nested.join("missing/child");
    assert_eq!(
        discover_project_root(&nonexistent).expect("nonexistent path should search its ancestors"),
        repo.path()
    );
}

#[test]
fn nested_project_wins_even_when_its_tasks_are_invalid() {
    let outer = task_repo("CORE-101", "Outer project task");
    let nested = outer.path().join("nested");
    fs::create_dir_all(nested.join(".tasks/core")).expect("nested project should exist");
    fs::write(
        nested.join(".tasks/core/CORE-102-invalid.md"),
        "invalid task content",
    )
    .expect("invalid task should be written");
    let work_dir = nested.join("work");
    fs::create_dir_all(&work_dir).expect("work directory should exist");

    assert_eq!(
        discover_project_root(&work_dir).expect("nested project should be discovered"),
        nested
    );

    let output = task_command(&work_dir, ["validate"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("task validation failed"), "{stderr}");
    assert!(!stderr.contains("could not find project"), "{stderr}");
}

#[test]
fn regular_tasks_file_is_not_a_project_marker() {
    let temp = tempfile::tempdir().expect("temp directory should exist");
    let start = temp.path().join("nested");
    fs::create_dir_all(&start).expect("start directory should exist");
    fs::write(start.join(".tasks"), "not a directory").expect("marker file should exist");

    let error = discover_project_root(&start).expect_err("regular file must not mark a project");
    assert!(
        matches!(error, TaskError::ProjectNotFound { start: error_start } if error_start == start)
    );
}

#[test]
fn missing_project_error_contains_the_original_search_start() {
    let temp = tempfile::tempdir().expect("temp directory should exist");
    let start = temp.path().join("nested");
    fs::create_dir_all(&start).expect("start directory should exist");

    let output = task_command(&start, ["validate"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    assert!(stderr.contains(&start.display().to_string()), "{stderr}");
    assert!(stderr.contains(".tasks directory"), "{stderr}");
}

#[test]
fn relative_and_absolute_roots_resolve_from_the_command_working_directory() {
    let temp = tempfile::tempdir().expect("temp directory should exist");
    let repo = temp.path().join("projects/alpha");
    create_task_repo(&repo, "CORE-101", "Alpha project task");
    let nested = repo.join("src/deep");
    fs::create_dir_all(&nested).expect("nested directory should exist");
    let unrelated = temp.path().join("unrelated/work");
    fs::create_dir_all(&unrelated).expect("unrelated directory should exist");

    let relative = Path::new("../../projects/alpha/src/deep");
    let relative_output = task_command_with_root(&unrelated, relative);
    assert_task_id(relative_output, "CORE-101");

    let absolute_output = task_command_with_root(&unrelated, &nested);
    assert_task_id(absolute_output, "CORE-101");
}

#[test]
fn commands_without_root_keep_independent_projects_isolated() {
    let first = task_repo("CORE-101", "First project task");
    let second = task_repo("CORE-202", "Second project task");
    let first_work = first.path().join("src/deep");
    let second_work = second.path().join("src/deep");
    fs::create_dir_all(&first_work).expect("first work directory should exist");
    fs::create_dir_all(&second_work).expect("second work directory should exist");

    assert_task_id(
        task_command(&first_work, ["list", "--format", "json"]),
        "CORE-101",
    );
    assert_task_id(
        task_command(&second_work, ["list", "--format", "json"]),
        "CORE-202",
    );
}

#[test]
fn command_help_explains_project_discovery_and_root_paths() {
    let output = Command::new(env!("CARGO_BIN_EXE_taskroot"))
        .args(["validate", "--help"])
        .output()
        .expect("help command should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(
        stdout.contains("defaults to the current directory"),
        "{stdout}"
    );
    assert!(stdout.contains("paths nested inside a project"), "{stdout}");
}

#[test]
fn compatibility_schema_file_is_ignored() {
    let repo = task_repo("CORE-101", "Compatibility file task");
    fs::write(
        repo.path().join(".tasks/task.schema.json"),
        "not schema content",
    )
    .expect("compatibility file should be written");

    let output = task_command(repo.path(), ["validate"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn task_repo(id: &str, title: &str) -> TempDir {
    let temp = tempfile::tempdir().expect("temp directory should exist");
    create_task_repo(temp.path(), id, title);
    temp
}

fn create_task_repo(root: &Path, id: &str, title: &str) {
    fs::create_dir_all(root.join(".tasks/core")).expect("task directory should exist");
    fs::write(
        root.join(".tasks/core").join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task should be written");
}

fn task_command<const N: usize>(current_dir: &Path, args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_taskroot"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("taskroot command should run")
}

fn task_command_with_root(current_dir: &Path, root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_taskroot"))
        .args(["list", "--root"])
        .arg(root)
        .args(["--format", "json"])
        .current_dir(current_dir)
        .output()
        .expect("taskroot command should run")
}

fn assert_task_id(output: std::process::Output, expected: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tasks: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    let tasks = tasks.as_array().expect("task output should be an array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], expected);
}
