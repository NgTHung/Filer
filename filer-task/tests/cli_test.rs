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
    assert_eq!(
        stdout,
        "Validation\nStatus: Passed\nTasks: 1\nWarnings: 0\n"
    );
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
    assert_eq!(json[0]["qualified_id"], "core:CORE-001");
    assert_eq!(json[0]["type"], "Feature");
    assert_eq!(json[0]["rules"][0], "CORE-LIBRARY");
    assert_eq!(json[0]["risk"], "High");
    assert_eq!(json[0]["impact"], "Touches validation and output");
    assert!(
        json[0].get("depends_on").is_none(),
        "empty depends_on should be omitted from JSON"
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
fn list_command_aligns_human_task_columns() {
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
    write_task(
        &repo,
        "core/CORE-002-cache-policy.md",
        "CORE-002",
        "Cache policy",
        "In Progress",
        "Medium",
        "",
    );

    let stdout = run_task_command_stdout(&repo, ["list"]);
    let lines: Vec<&str> = stdout.lines().collect();
    let status_column = lines[0].find("STATUS").expect("status header should exist");

    assert_eq!(lines.len(), 4);
    assert!(
        lines[1]
            .chars()
            .all(|character| character == '-' || character == ' ')
    );
    assert!(lines[2][status_column..].starts_with("To Do"));
    assert!(lines[3][status_column..].starts_with("In Progress"));
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
            "core:CORE-042",
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
fn deps_command_uses_aligned_human_table() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-001-open-dependency.md",
        "CORE-001",
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
        "depends_on: [CORE-001]\n",
    );

    let stdout = run_task_command_stdout(&repo, ["deps", "core:CORE-042"]);
    let lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("TASK"));
    assert!(
        lines[1]
            .chars()
            .all(|character| character == '-' || character == ' ')
    );
    assert!(lines[2].starts_with("core:CORE-001"));
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
    let lines: Vec<&str> = stdout.lines().collect();
    let heading = lines
        .iter()
        .position(|line| *line == "Open Tasks")
        .expect("open tasks heading should exist");
    let header = lines[heading + 1];
    let status_column = header.find("STATUS").expect("status header should exist");
    let task_line = lines
        .iter()
        .find(|line| line.starts_with("core:CORE-042"))
        .expect("open task should exist");

    assert!(header.starts_with("TASK"));
    assert!(
        lines[heading + 2]
            .chars()
            .all(|character| character == '-' || character == ' ')
    );
    assert!(task_line[status_column..].starts_with("To Do"));
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
    assert_eq!(
        String::from_utf8(add.stdout).expect("stdout should be valid UTF-8"),
        "Task Created\nTask: core:CORE-042\nPath: .tasks/core/CORE-042-provider-timeout-propagation.md\n"
    );

    assert_eq!(
        run_task_command_stdout(&repo, ["start", "core:CORE-042"]),
        "Task Started\nTask: core:CORE-042\nPath: .tasks/core/CORE-042-provider-timeout-propagation.md\n"
    );
    let task_path = repo
        .path()
        .join(".tasks/core/CORE-042-provider-timeout-propagation.md");
    let started = fs::read_to_string(&task_path).expect("task should be readable");
    assert!(started.contains("status: In Progress"));

    assert_eq!(
        run_task_command_stdout(
            &repo,
            [
                "block",
                "core:CORE-042",
                "Waiting for provider timeout policy decision.",
            ],
        ),
        "Task Blocked\nTask: core:CORE-042\nPath: .tasks/core/CORE-042-provider-timeout-propagation.md\n"
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
    assert_eq!(
        run_task_command_stdout(&repo, ["done", "core:CORE-042"]),
        "Task Completed\nTask: core:CORE-042\nPath: .tasks/core/CORE-042-provider-timeout-propagation.md\n"
    );
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
    assert_eq!(
        run_task_command_stdout(
            &repo,
            [
                "defer",
                "core:CORE-043",
                "No longer needed for this milestone.",
            ],
        ),
        "Task Deferred\nTask: core:CORE-043\nPath: .tasks/core/CORE-043-cache-policy.md\n"
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
    assert_eq!(
        run_task_command_stdout(
            &repo,
            ["obsolete", "core:CORE-044", "Replaced by CORE-045."],
        ),
        "Task Obsolete\nTask: core:CORE-044\nPath: .tasks/core/CORE-044-replaced-policy.md\n"
    );
    let obsolete = fs::read_to_string(repo.path().join(".tasks/core/CORE-044-replaced-policy.md"))
        .expect("task should be readable");
    assert!(obsolete.contains("status: Obsolete"));
    assert!(obsolete.contains("## Rationale"));
}

#[test]
fn add_command_creates_milestone_and_rich_task() {
    let repo = task_repo();

    run_task_command(
        &repo,
        [
            "add",
            "--domain",
            "milestones",
            "--id",
            "milestones:MILESTONE-003",
            "--title",
            "Core contract stabilization",
            "--priority",
            "High",
            "--type",
            "Milestone",
            "--milestone",
            "0.3.0",
            "--criterion",
            "Public contracts are named consistently.",
        ],
    );
    run_task_command(
        &repo,
        [
            "add",
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
            "--parent",
            "milestones:MILESTONE-003",
            "--milestone",
            "0.3.0",
            "--rule",
            "PROVIDER-ACCESS",
            "--risk",
            "High",
            "--impact",
            "Touches provider calls and cancellation behavior.",
            "--tag",
            "provider",
            "--summary",
            "Propagate provider deadlines through core calls.",
            "--criterion",
            "Provider calls receive timeout context.",
        ],
    );

    run_task_command(&repo, ["validate"]);
    let task = fs::read_to_string(
        repo.path()
            .join(".tasks/core/CORE-042-provider-timeout-propagation.md"),
    )
    .expect("task should be readable");
    assert!(task.contains("parent: \"milestones:MILESTONE-003\""));
    assert!(task.contains("rules: [\"PROVIDER-ACCESS\"]"));
    assert!(task.contains("tags: [\"provider\"]"));
    assert!(task.contains("## Acceptance Criteria"));
}

#[test]
fn import_command_dry_run_writes_no_files() {
    let repo = task_repo();
    let manifest = write_import_manifest(
        &repo,
        r#"[
  {
    "domain": "milestones",
    "id": "MILESTONE-003",
    "title": "Core contract stabilization",
    "priority": "High",
    "type": "Milestone",
    "milestone": "0.3.0",
    "criteria": [{ "text": "Contracts are stable." }]
  }
]"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "import",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
        ])
        .arg(&manifest)
        .arg("--dry-run")
        .output()
        .expect("import command should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be valid UTF-8"),
        "Import\nMode: Dry Run\nTasks: 1\n\nPaths\n.tasks/milestones/MILESTONE-003-core-contract-stabilization.md\n"
    );
    assert!(!repo.path().join(".tasks/milestones").exists());
}

#[test]
fn import_command_writes_batch_and_allows_skip_existing_rerun() {
    let repo = task_repo();
    let manifest = write_import_manifest(
        &repo,
        r#"[
  {
    "domain": "milestones",
    "id": "MILESTONE-003",
    "title": "Core contract stabilization",
    "priority": "High",
    "type": "Milestone",
    "milestone": "0.3.0",
    "criteria": [{ "text": "Contracts are stable." }]
  },
  {
    "domain": "core",
    "id": "CORE-042",
    "title": "Provider timeout propagation",
    "priority": "High",
    "type": "Feature",
    "parent": "milestones:MILESTONE-003",
    "milestone": "0.3.0",
    "rules": ["PROVIDER-ACCESS"],
    "risk": "High",
    "impact": "Touches provider calls and cancellation behavior.",
    "tags": ["provider"],
    "summary": "Propagate provider deadlines through core calls.",
    "criteria": [{ "text": "Provider calls receive timeout context." }]
  }
]"#,
    );

    run_import_command(&repo, &manifest, false, false);
    run_task_command(&repo, ["validate"]);
    run_import_command(&repo, &manifest, false, true);
}

#[test]
fn import_command_rejects_invalid_references_before_writing() {
    let repo = task_repo();
    let manifest = write_import_manifest(
        &repo,
        r#"[
  {
    "domain": "core",
    "id": "CORE-042",
    "title": "Provider timeout propagation",
    "priority": "High",
    "type": "Feature",
    "parent": "CORE-999",
    "criteria": [{ "text": "Provider calls receive timeout context." }]
  }
]"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "import",
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
        ])
        .arg(&manifest)
        .output()
        .expect("import command should run");

    assert!(!output.status.success());
    assert!(
        !repo
            .path()
            .join(".tasks/core")
            .join("CORE-042-provider-timeout-propagation.md")
            .exists()
    );
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

fn run_import_command(
    repo: &TempDir,
    manifest: &std::path::Path,
    dry_run: bool,
    skip_existing: bool,
) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_filer-task"));
    command.args([
        "import",
        "--root",
        repo.path().to_str().expect("temp path should be UTF-8"),
    ]);
    command.arg(manifest);
    if dry_run {
        command.arg("--dry-run");
    }
    if skip_existing {
        command.arg("--skip-existing");
    }
    let output = command.output().expect("import command should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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

fn run_task_command_stdout<const N: usize>(repo: &TempDir, args: [&str; N]) -> String {
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
    String::from_utf8(output.stdout).expect("stdout should be valid UTF-8")
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

fn write_import_manifest(repo: &TempDir, content: &str) -> std::path::PathBuf {
    let path = repo.path().join("roadmap-import.json");
    fs::write(&path, content).expect("manifest should be written");
    path
}
