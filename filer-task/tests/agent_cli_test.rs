use std::{fs, process::Command};

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn show_returns_structured_task_body_and_complete_human_metadata() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-001-location-routing.md",
        "CORE-001",
        "Location routing",
        "To Do",
        "High",
        "tags: [location]\n",
    );
    append_section(
        &repo,
        "core/CORE-001-location-routing.md",
        "## Notes\n\nKeep provider boundaries explicit.\n",
    );

    let json = run_json(&repo, ["show", "CORE-001", "--format", "json"]);
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["detail"]["task"]["id"], "CORE-001");
    assert_eq!(
        json["detail"]["task"]["path"],
        ".tasks/core/CORE-001-location-routing.md"
    );
    assert_eq!(json["detail"]["criteria"][0]["text"], "Works");

    let sections = json["detail"]["sections"]
        .as_array()
        .expect("sections should be an array");
    assert_eq!(sections[0]["heading"], "Notes");
    assert!(
        sections
            .iter()
            .all(|section| section["heading"] != "Acceptance Criteria"),
        "criteria heading must not be duplicated in sections"
    );

    let task = &json["detail"]["task"];
    assert!(task.get("parent").is_none(), "absent parent should be omitted");
    assert!(
        task.get("milestone").is_none(),
        "absent milestone should be omitted"
    );
    assert!(
        task.get("whitepaper").is_none(),
        "absent whitepaper should be omitted"
    );
    assert!(
        task.get("depends_on").is_none(),
        "empty depends_on should be omitted"
    );

    let human = run_stdout(&repo, ["show", "CORE-001"]);
    assert!(human.contains("Dependencies: -"));
    assert!(human.contains("Rules: CORE-LIBRARY"));
    assert!(human.contains("Tags: location"));
    assert!(human.contains("Notes\nKeep provider boundaries explicit."));
    assert!(
        human.contains("Acceptance Criteria\n- [ ] Works"),
        "human output should still render the criteria checklist"
    );
}

#[test]
fn ready_returns_filtered_priority_ordered_leaves_in_both_formats() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-001-parent.md",
        "CORE-001",
        "Parent task",
        "In Progress",
        "High",
        "",
    );
    write_task(
        &repo,
        "core/CORE-002-low-ready.md",
        "CORE-002",
        "Low ready task",
        "To Do",
        "Low",
        "parent: CORE-001\ntags: [agent]\n",
    );
    write_task(
        &repo,
        "core/CORE-003-high-ready.md",
        "CORE-003",
        "High ready task",
        "To Do",
        "High",
        "parent: CORE-001\ntags: [agent]\n",
    );
    write_task(
        &repo,
        "core/CORE-004-blocked-by-dependency.md",
        "CORE-004",
        "Blocked by dependency",
        "To Do",
        "High",
        "depends_on: [CORE-003]\ntags: [agent]\n",
    );

    let args = [
        "ready", "--domain", "core", "--tag", "agent", "--limit", "1",
    ];
    let human = run_stdout(&repo, args);
    let lines: Vec<&str> = human.lines().collect();
    let status_column = lines[0].find("STATUS").expect("status header should exist");
    let title_column = lines[0].find("TITLE").expect("title header should exist");

    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("ID"));
    assert!(
        lines[1]
            .chars()
            .all(|character| character == '-' || character == ' ')
    );
    assert!(lines[2].starts_with("CORE-003"));
    assert!(lines[2][status_column..].starts_with("To Do"));
    assert!(lines[2][title_column..].starts_with("High ready task"));

    let json = run_json(
        &repo,
        [
            "ready", "--domain", "core", "--tag", "agent", "--limit", "1", "--format", "json",
        ],
    );
    assert_eq!(json["schema_version"], 1);
    assert_eq!(
        json["tasks"]
            .as_array()
            .expect("tasks should be an array")
            .len(),
        1
    );
    assert_eq!(json["tasks"][0]["id"], "CORE-003");
}

#[test]
fn context_returns_relations_readiness_rules_and_human_sections() {
    let repo = task_repo();
    write_milestone(&repo, "MILESTONE-003", "0.3.0");
    write_task(
        &repo,
        "core/CORE-001-parent.md",
        "CORE-001",
        "Parent task",
        "In Progress",
        "High",
        "milestone: \"0.3.0\"\n",
    );
    write_checked_task(
        &repo,
        "core/CORE-002-done-dependency.md",
        "CORE-002",
        "Done dependency",
        "Done",
        "",
    );
    write_task(
        &repo,
        "core/CORE-003-target.md",
        "CORE-003",
        "Target task",
        "To Do",
        "High",
        "parent: CORE-001\nmilestone: \"0.3.0\"\ndepends_on: [CORE-002]\nwhitepaper: docs/design.md\n",
    );
    write_task(
        &repo,
        "core/CORE-004-dependent.md",
        "CORE-004",
        "Dependent task",
        "To Do",
        "Medium",
        "depends_on: [CORE-003]\n",
    );

    let json = run_json(&repo, ["context", "CORE-003", "--format", "json"]);
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["readiness"]["ready"], true);
    assert_eq!(json["parent"]["task"]["id"], "CORE-001");
    assert_eq!(json["dependencies"][0]["task"]["id"], "CORE-002");
    assert_eq!(json["dependents"][0]["task"]["id"], "CORE-004");
    assert_eq!(json["dependents"][0]["readiness"]["ready"], false);
    assert_eq!(json["milestone"]["task"]["id"], "MILESTONE-003");
    assert_eq!(json["rules"][0]["id"], "CORE-LIBRARY");
    assert!(
        json["rules"][0]["text"]
            .as_str()
            .expect("rule text should be a string")
            .contains("must not depend on GUI frameworks")
    );
    assert_eq!(json["whitepaper"], "docs/design.md");

    let human = run_stdout(&repo, ["context", "CORE-003"]);
    assert!(human.contains("Readiness\nReady"));
    assert!(human.contains("Dependencies\nCORE-002"));
    assert!(human.contains("Rules\nCORE-LIBRARY"));
}

#[test]
fn detail_commands_reject_unknown_tasks_and_missing_rule_sections() {
    let repo = task_repo();
    let unknown = run_output(&repo, ["show", "CORE-999", "--format", "json"]);
    assert!(!unknown.status.success());
    assert!(unknown.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("task CORE-999 does not exist"));

    write_task(
        &repo,
        "core/CORE-001-location-routing.md",
        "CORE-001",
        "Location routing",
        "To Do",
        "High",
        "",
    );
    fs::write(
        repo.path().join("docs/architecture/invariants.md"),
        "# Architecture Invariants\n",
    )
    .expect("invariants should be replaced");
    let missing_rule = run_output(&repo, ["context", "CORE-001", "--format", "json"]);
    assert!(!missing_rule.status.success());
    assert!(
        String::from_utf8_lossy(&missing_rule.stderr)
            .contains("rule CORE-LIBRARY does not have a section")
    );
}

#[test]
fn context_reports_dependency_and_ancestor_blockers() {
    let repo = task_repo();
    write_task(
        &repo,
        "core/CORE-001-blocked-parent.md",
        "CORE-001",
        "Blocked parent",
        "Blocked",
        "High",
        "",
    );
    append_section(
        &repo,
        "core/CORE-001-blocked-parent.md",
        "## Blocked Reason\n\nWaiting for a design decision.\n",
    );
    write_task(
        &repo,
        "core/CORE-002-open-dependency.md",
        "CORE-002",
        "Open dependency",
        "In Progress",
        "High",
        "",
    );
    write_task(
        &repo,
        "core/CORE-003-target.md",
        "CORE-003",
        "Target task",
        "To Do",
        "High",
        "parent: CORE-001\ndepends_on: [CORE-002]\n",
    );

    let json = run_json(&repo, ["context", "CORE-003", "--format", "json"]);
    let blockers = json["readiness"]["blockers"]
        .as_array()
        .expect("blockers should be an array");

    assert_eq!(json["readiness"]["ready"], false);
    assert_eq!(blockers[0]["kind"], "dependency");
    assert_eq!(blockers[0]["task_id"], "CORE-002");
    assert_eq!(blockers[1]["kind"], "ancestor_status");
    assert_eq!(blockers[1]["task_id"], "CORE-001");
}

fn task_repo() -> TempDir {
    let temp = tempfile::tempdir().expect("temp dir should be created");
    fs::create_dir_all(temp.path().join(".tasks/core")).expect("core task dir should exist");
    fs::create_dir_all(temp.path().join("docs/architecture"))
        .expect("architecture docs dir should exist");
    fs::write(temp.path().join(".tasks/task.schema.json"), "{}")
        .expect("schema marker should exist");
    fs::write(
        temp.path().join("docs/architecture/invariants.md"),
        "# Architecture Invariants\n\n## CORE-LIBRARY\n\n`filer-core` must not depend on GUI frameworks.\n",
    )
    .expect("invariants should exist");
    temp
}

fn run_json<const N: usize>(repo: &TempDir, args: [&str; N]) -> Value {
    serde_json::from_str(&run_stdout(repo, args)).expect("stdout should be JSON")
}

fn run_stdout<const N: usize>(repo: &TempDir, args: [&str; N]) -> String {
    let output = run_output(repo, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be valid UTF-8")
}

fn run_output<const N: usize>(repo: &TempDir, args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .arg(args[0])
        .args([
            "--root",
            repo.path().to_str().expect("temp path should be UTF-8"),
        ])
        .args(&args[1..])
        .output()
        .expect("command should run")
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
