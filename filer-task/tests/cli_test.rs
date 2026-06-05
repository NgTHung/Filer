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
    assert_eq!(json[0]["type"], "Feature");
    assert_eq!(json[0]["rules"][0], "CORE-LIBRARY");
    assert_eq!(json[0]["risk"], "High");
    assert_eq!(json[0]["impact"], "Touches validation and output");
    assert_eq!(
        json[0]["depends_on"]
            .as_array()
            .expect("depends_on should be an array")
            .len(),
        0
    );
}

#[test]
fn list_command_filters_by_milestone_and_blocked() {
    let repo = task_repo();
    write_milestone(&repo, "MILESTONE-003", "0.3.0");
    write_task(
        &repo,
        "core/CORE-042-timeout-propagation.md",
        "CORE-042",
        "Timeout propagation",
        "Blocked",
        "High",
        "milestone: \"0.3.0\"\n",
    );
    append_section(
        &repo,
        "core/CORE-042-timeout-propagation.md",
        "## Blocked Reason\n\nWaiting for policy.\n",
    );
    write_task(
        &repo,
        "core/CORE-043-cache-policy.md",
        "CORE-043",
        "Cache policy",
        "To Do",
        "High",
        "",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "list",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
            "--milestone",
            "0.3.0",
            "--blocked",
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
    assert_eq!(json[0]["id"], "CORE-042");
    assert_eq!(json[0]["milestone"], "0.3.0");
}

#[test]
fn deps_command_outputs_incomplete_dependencies() {
    let repo = task_repo();
    write_checked_task(
        &repo,
        "core/CORE-001-done-dependency.md",
        "CORE-001",
        "Done dependency",
        "Done",
        "",
    );
    write_task(
        &repo,
        "core/CORE-002-open-dependency.md",
        "CORE-002",
        "Open dependency",
        "To Do",
        "High",
        "",
    );
    write_task(
        &repo,
        "core/CORE-042-timeout-propagation.md",
        "CORE-042",
        "Timeout propagation",
        "To Do",
        "High",
        "depends_on: [CORE-001, CORE-002]\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "deps",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
            "--incomplete",
            "CORE-042",
            "--format",
            "json",
        ])
        .output()
        .expect("deps command should run");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        json.as_array()
            .expect("JSON output should be an array")
            .len(),
        1
    );
    assert_eq!(json[0]["id"], "CORE-002");
}

#[test]
fn milestone_command_outputs_exit_checklist() {
    let repo = task_repo();
    write_milestone(&repo, "MILESTONE-003", "0.3.0");
    write_task(
        &repo,
        "core/CORE-042-timeout-propagation.md",
        "CORE-042",
        "Timeout propagation",
        "To Do",
        "High",
        "milestone: \"0.3.0\"\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "milestone",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
            "0.3.0",
            "--exit-checklist",
        ])
        .output()
        .expect("milestone command should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(stdout.contains("- [ ] Finish milestone"));
    assert!(stdout.contains("CORE-042"));
}

#[test]
fn summary_command_emits_count_maps() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-042-timeout-propagation.md",
        "CORE-042",
        "Timeout propagation",
        "To Do",
        "High",
        "",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "summary",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("summary command should run");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(json["status"]["To Do"], 1);
    assert_eq!(json["domain"]["core"], 1);
    assert_eq!(json["priority"]["High"], 1);
}

#[test]
fn lifecycle_commands_update_task_files() {
    let repo = task_repo();
    write_milestone(&repo, "MILESTONE-003", "0.3.0");

    let add = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "add",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
            "--domain",
            "core",
            "--id",
            "CORE-042",
            "--title",
            "Provider timeout propagation",
            "--priority",
            "High",
            "--type",
            "Feature",
            "--milestone",
            "0.3.0",
        ])
        .output()
        .expect("add command should run");
    assert!(add.status.success());

    run_task_command(&repo, ["start", "CORE-042"]);
    let task_path = repo
        .path()
        .join(".tasks/core/CORE-042-provider-timeout-propagation.md");
    let started = fs::read_to_string(&task_path).expect("task should be readable");
    assert!(started.contains("status: In Progress"));

    run_task_command(
        &repo,
        [
            "block",
            "CORE-042",
            "Waiting for provider timeout policy decision.",
        ],
    );
    let blocked = fs::read_to_string(&task_path).expect("task should be readable");
    assert!(blocked.contains("status: Blocked"));
    assert!(blocked.contains("## Blocked Reason"));

    fs::write(
        &task_path,
        blocked.replace(
            "- [ ] Define completion criteria.",
            "- [x] Define completion criteria.",
        ),
    )
    .expect("task should be updated");
    run_task_command(&repo, ["done", "CORE-042"]);
    let done = fs::read_to_string(&task_path).expect("task should be readable");
    assert!(done.contains("status: Done"));

    write_task(
        &repo,
        "core/CORE-043-cache-policy.md",
        "CORE-043",
        "Cache policy",
        "To Do",
        "High",
        "",
    );
    run_task_command(
        &repo,
        ["defer", "CORE-043", "No longer needed for this milestone."],
    );
    let deferred = fs::read_to_string(repo.path().join(".tasks/core/CORE-043-cache-policy.md"))
        .expect("task should be readable");
    assert!(deferred.contains("status: Deferred"));
    assert!(deferred.contains("## Rationale"));

    write_task(
        &repo,
        "core/CORE-044-replaced-policy.md",
        "CORE-044",
        "Replaced policy",
        "To Do",
        "High",
        "",
    );
    run_task_command(&repo, ["obsolete", "CORE-044", "Replaced by CORE-045."]);
    let obsolete = fs::read_to_string(repo.path().join(".tasks/core/CORE-044-replaced-policy.md"))
        .expect("task should be readable");
    assert!(obsolete.contains("status: Obsolete"));
    assert!(obsolete.contains("## Rationale"));
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

fn run_task_command<const N: usize>(repo: &TempDir, args: [&str; N]) {
    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .arg(args[0])
        .args([
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
        ])
        .args(&args[1..])
        .output()
        .expect("command should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\ntype: Feature\nrules: [CORE-LIBRARY]\nrisk: High\nimpact: Touches validation and output\n{extra}---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task should be written");
}

fn write_checked_task(
    repo: &TempDir,
    relative: &str,
    id: &str,
    title: &str,
    status: &str,
    extra: &str,
) {
    fs::write(
        repo.path().join(".tasks").join(relative),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: High\ntype: Feature\nrules: [CORE-LIBRARY]\nrisk: High\nimpact: Touches validation and output\n{extra}---\n\n## Acceptance Criteria\n\n- [x] Works\n"
        ),
    )
    .expect("task should be written");
}

fn write_milestone(repo: &TempDir, id: &str, milestone: &str) {
    fs::create_dir_all(repo.path().join(".tasks/milestones"))
        .expect("milestone task dir should exist");
    fs::write(
        repo.path()
            .join(".tasks/milestones")
            .join(format!("{id}-project-milestone.md")),
        format!(
            "---\nid: {id}\ntitle: Project milestone\nstatus: To Do\npriority: High\ntype: Milestone\nmilestone: \"{milestone}\"\n---\n\n## Exit Criteria\n\n- [ ] Finish milestone\n"
        ),
    )
    .expect("milestone should be written");
}

fn append_section(repo: &TempDir, relative: &str, section: &str) {
    let path = repo.path().join(".tasks").join(relative);
    let mut content = fs::read_to_string(&path).expect("task should be readable");
    content.push('\n');
    content.push_str(section);
    fs::write(path, content).expect("task should be updated");
}
