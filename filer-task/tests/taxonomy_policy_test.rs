use std::{fs, path::Path, process::Command};

use filer_task::{
    agent_context::{ReadyFilter, build_context, build_ready},
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{Criterion, NewTask, add_task, import_tasks},
    model::{Priority, TaskStatus, TaskType},
    project::TaskProject,
    validate::{require_valid_report, validate_repo},
};
use serde_json::Value;

fn project(config: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temp project created");
    fs::create_dir(temp.path().join(".tasks")).expect("task directory created");
    fs::write(temp.path().join(".tasks/config.json"), config).expect("configuration written");
    temp
}

fn config(tags: &str) -> String {
    format!(
        r#"{{
  "version": 1,
  "domains": {{
    "backend": {{"prefixes": ["WORK", "API"]}},
    "release": {{"prefixes": ["WORK", "REL"]}}
  }},
  "task_types": {{
    "Change": {{"criteria": "acceptance"}},
    "Container": {{"criteria": "exit"}},
    "ReleaseGate": {{"criteria": "acceptance", "role": "milestone"}}
  }},
  "tags": {tags}
}}"#
    )
}

fn write_task(root: &Path, domain: &str, id: &str, task_type: &str, extra: &str, heading: &str) {
    let directory = root.join(".tasks").join(domain);
    fs::create_dir_all(&directory).expect("domain directory created");
    fs::write(
        directory.join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: Taxonomy task\nstatus: To Do\npriority: High\ntype: {task_type}\n{extra}---\n\n## Summary\n\nExercise taxonomy policy.\n\n## {heading}\n\n- [ ] Works.\n"
        ),
    )
    .expect("task written");
}

fn new_task(domain: &str, id: &str, task_type: &str) -> NewTask {
    NewTask {
        domain: domain.to_string(),
        id: id.to_string(),
        title: "Taxonomy task".to_string(),
        status: TaskStatus::ToDo,
        priority: Priority::High,
        task_type: TaskType::new(task_type),
        parent: None,
        milestone: None,
        depends_on: Vec::new(),
        rules: Vec::new(),
        risk: None,
        impact: None,
        tags: Vec::new(),
        whitepaper: None,
        summary: Some("Exercise taxonomy policy.".to_string()),
        criteria: vec![Criterion {
            text: "Works.".to_string(),
            checked: false,
        }],
        rationale: None,
        blocked_reason: None,
    }
}

#[test]
fn duplicate_creation_has_a_stable_reason_code() {
    let temp = project(&config(r#"{"policy": "open"}"#));
    let project = TaskProject::open(temp.path()).expect("project opens");
    add_task(&project, new_task("backend", "WORK-001", "Change")).expect("task created");

    let error = add_task(&project, new_task("backend", "WORK-001", "Change"))
        .expect_err("duplicate rejected");

    assert_eq!(error.code(), "id_exists");
    assert_eq!(error.context()["task"], "backend:WORK-001");
}

#[test]
fn prefixes_are_domain_scoped_and_report_structured_failures() {
    let temp = project(&config(r#"{"policy": "open"}"#));
    write_task(
        temp.path(),
        "backend",
        "WORK-001",
        "Change",
        "",
        "Acceptance Criteria",
    );
    write_task(
        temp.path(),
        "release",
        "WORK-001",
        "Change",
        "",
        "Acceptance Criteria",
    );
    write_task(
        temp.path(),
        "release",
        "API-002",
        "Change",
        "",
        "Acceptance Criteria",
    );

    let project = TaskProject::open(temp.path()).expect("project opens");
    let report = validate_repo(&project).expect("repository scans");
    assert_eq!(report.tasks.len(), 3);
    let issue = report
        .errors
        .iter()
        .find(|issue| issue.code == "prefix_not_allowed")
        .expect("stored task prefix is rejected");
    assert_eq!(issue.context["domain"], "release");
    assert_eq!(issue.context["field"], "id");
    assert_eq!(issue.context["rejected_value"], "API");

    fs::remove_file(temp.path().join(".tasks/release/API-002-task.md"))
        .expect("invalid fixture removed");
    let project = TaskProject::open(temp.path()).expect("clean project reopens");

    let error = add_task(&project, new_task("backend", "REL-003", "Change"))
        .expect_err("add rejects a domain-specific prefix");
    assert_eq!(error.code(), "prefix_not_allowed");
    assert_eq!(error.context()["task"], "backend:REL-003");
    assert_eq!(
        error.context()["allowed"],
        serde_json::json!(["WORK", "API"])
    );

    let wrapped = TaskError::Validation(report.errors);
    assert_eq!(wrapped.context()["issues"][0]["code"], "prefix_not_allowed");
}

#[test]
fn configured_types_round_trip_and_drive_criteria() {
    let temp = project(&config(r#"{"policy": "open"}"#));
    write_task(
        temp.path(),
        "backend",
        "WORK-001",
        "Change",
        "",
        "Acceptance Criteria",
    );
    write_task(
        temp.path(),
        "backend",
        "WORK-002",
        "Container",
        "",
        "Exit Criteria",
    );
    let project = TaskProject::open(temp.path()).expect("project opens");
    let validated = require_valid_report(validate_repo(&project).expect("repository scans"))
        .expect("configured types validate");
    assert_eq!(validated[0].metadata.task_type.as_str(), "Change");
    assert_eq!(validated[1].metadata.task_type.to_string(), "Container");
    assert_eq!(
        serde_json::to_value(&validated[1].metadata.task_type).unwrap(),
        "Container"
    );

    let created = add_task(&project, new_task("backend", "WORK-003", "Container"))
        .expect("custom container type is accepted");
    assert!(
        fs::read_to_string(created)
            .expect("created task read")
            .contains("## Exit Criteria")
    );

    let cli_add = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "add",
            "--domain",
            "backend",
            "--id",
            "WORK-006",
            "--title",
            "CLI custom type",
            "--priority",
            "High",
            "--type",
            "Change",
            "--root",
        ])
        .arg(temp.path())
        .output()
        .expect("custom type CLI add runs");
    assert!(cli_add.status.success());
    let cli_list = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["list", "--format", "json", "--root"])
        .arg(temp.path())
        .output()
        .expect("custom type CLI list runs");
    let listed: Value = serde_json::from_slice(&cli_list.stdout).expect("list output is JSON");
    assert!(
        listed
            .as_array()
            .expect("task array")
            .iter()
            .any(|task| { task["qualified_id"] == "backend:WORK-006" && task["type"] == "Change" })
    );

    write_task(
        temp.path(),
        "backend",
        "WORK-004",
        "Mystery",
        "",
        "Acceptance Criteria",
    );
    let report = validate_repo(&project).expect("repository scans");
    let issue = report
        .errors
        .iter()
        .find(|issue| issue.code == "unknown_type")
        .expect("stored unknown type is rejected");
    assert_eq!(issue.context["rejected_value"], "Mystery");

    fs::remove_file(temp.path().join(".tasks/backend/WORK-004-task.md"))
        .expect("invalid fixture removed");
    let error = import_tasks(
        &project,
        &[new_task("backend", "WORK-005", "Mystery")],
        true,
        false,
    )
    .expect_err("import rejects unknown type");
    assert_eq!(error.code(), "unknown_type");
    assert_eq!(error.context()["field"], "type");
}

#[test]
fn milestone_role_is_name_and_domain_independent() {
    let temp = project(&config(r#"{"policy": "open"}"#));
    write_task(
        temp.path(),
        "release",
        "REL-001",
        "ReleaseGate",
        "milestone: \"2.0.0\"\n",
        "Acceptance Criteria",
    );
    write_task(
        temp.path(),
        "backend",
        "API-002",
        "Change",
        "milestone: \"2.0.0\"\n",
        "Acceptance Criteria",
    );
    let project = TaskProject::open(temp.path()).expect("project opens");
    let validated = require_valid_report(validate_repo(&project).expect("repository scans"))
        .expect("renamed milestone validates");
    let context = build_context(
        &project,
        &validated,
        &TaskIdentity::new("backend", "API-002").unwrap(),
        &[],
    )
    .expect("context builds");
    assert_eq!(
        context
            .milestone
            .expect("milestone relation")
            .task
            .qualified_id(),
        "release:REL-001"
    );
    let ready =
        build_ready(&project, &validated, &ReadyFilter::default(), &[]).expect("ready view builds");
    assert!(
        ready
            .tasks
            .iter()
            .all(|task| task.qualified_id() != "release:REL-001")
    );

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["milestone", "2.0.0", "--exit-checklist", "--root"])
        .arg(temp.path())
        .output()
        .expect("milestone command runs");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Acceptance Criteria\n- [ ] Works."));
}

#[test]
fn milestone_bindings_are_required_unique_and_project_wide() {
    let temp = project(&config(r#"{"policy": "open"}"#));
    write_task(
        temp.path(),
        "release",
        "REL-001",
        "ReleaseGate",
        "",
        "Acceptance Criteria",
    );
    write_task(
        temp.path(),
        "backend",
        "API-001",
        "Change",
        "milestone: \"missing\"\n",
        "Acceptance Criteria",
    );
    let project = TaskProject::open(temp.path()).expect("project opens");
    let report = validate_repo(&project).expect("repository scans");
    assert!(report.errors.iter().any(|issue| {
        issue.message.contains("milestone-role task") && issue.message.contains("non-empty")
    }));
    assert!(report.errors.iter().any(|issue| {
        issue.message.contains("missing") && issue.message.contains("exactly one")
    }));

    fs::remove_file(temp.path().join(".tasks/release/REL-001-task.md"))
        .expect("missing milestone fixture removed");
    write_task(
        temp.path(),
        "release",
        "REL-002",
        "ReleaseGate",
        "milestone: \"duplicate\"\n",
        "Acceptance Criteria",
    );
    write_task(
        temp.path(),
        "backend",
        "API-002",
        "ReleaseGate",
        "milestone: \"duplicate\"\n",
        "Acceptance Criteria",
    );
    let report = validate_repo(&project).expect("repository scans");
    assert!(report.errors.iter().any(|issue| {
        issue.message.contains("duplicate") && issue.message.contains("multiple")
    }));
}

#[test]
fn open_and_strict_tags_share_validation_across_reads_writes_and_filters() {
    let open = project(&config(r#"{"policy": "open"}"#));
    let open_project = TaskProject::open(open.path()).expect("open project opens");
    let mut accepted = new_task("backend", "WORK-001", "Change");
    accepted.tags = vec!["any-valid-tag".to_string()];
    add_task(&open_project, accepted).expect("open policy accepts a portable tag");
    let mut invalid = new_task("backend", "WORK-002", "Change");
    invalid.tags = vec!["Not Portable".to_string()];
    let error = add_task(&open_project, invalid).expect_err("open policy still checks syntax");
    assert_eq!(error.code(), "tag_rejected");

    let strict = project(&config(
        r#"{"policy": "strict", "allowed": ["backend", "release"]}"#,
    ));
    write_task(
        strict.path(),
        "backend",
        "WORK-001",
        "Change",
        "tags: [unknown]\n",
        "Acceptance Criteria",
    );
    let strict_project = TaskProject::open(strict.path()).expect("strict project opens");
    let report = validate_repo(&strict_project).expect("repository scans");
    let issue = report
        .errors
        .iter()
        .find(|issue| issue.code == "tag_rejected")
        .expect("stored tag rejected");
    assert_eq!(issue.context["rejected_value"], "unknown");
    assert_eq!(
        issue.context["allowed"],
        serde_json::json!(["backend", "release"])
    );

    fs::remove_file(strict.path().join(".tasks/backend/WORK-001-task.md"))
        .expect("invalid fixture removed");
    let strict_project = TaskProject::open(strict.path()).expect("clean strict project reopens");
    let mut rejected = new_task("backend", "WORK-002", "Change");
    rejected.tags = vec!["unknown".to_string()];
    assert_eq!(
        add_task(&strict_project, rejected)
            .expect_err("strict add rejects unknown tag")
            .code(),
        "tag_rejected"
    );
    let filter_error = build_ready(
        &strict_project,
        &[],
        &ReadyFilter {
            tag: Some("unknown".to_string()),
            ..ReadyFilter::default()
        },
        &[],
    )
    .expect_err("strict filter rejects unknown tag");
    assert_eq!(filter_error.code(), "tag_rejected");

    let empty = project(&config(r#"{"policy": "strict", "allowed": []}"#));
    let empty_project = TaskProject::open(empty.path()).expect("empty strict catalog opens");
    let mut rejected = new_task("backend", "WORK-003", "Change");
    rejected.tags = vec!["backend".to_string()];
    assert_eq!(
        add_task(&empty_project, rejected)
            .expect_err("empty strict catalog rejects every tag")
            .code(),
        "tag_rejected"
    );
}

#[test]
fn task_type_string_value_parses_displays_orders_and_serializes() {
    let parsed: TaskType = "CustomGate".parse().expect("custom type parses");
    assert_eq!(parsed.as_str(), "CustomGate");
    assert_eq!(parsed.to_string(), "CustomGate");
    assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""CustomGate""#);
    assert!(TaskType::new("Alpha") < TaskType::new("Beta"));

    let json: Value = serde_json::from_str(r#"{"type":"ImportedType"}"#).unwrap();
    let decoded: TaskType = serde_json::from_value(json["type"].clone()).unwrap();
    assert_eq!(decoded, TaskType::new("ImportedType"));
}

#[test]
fn cli_help_describes_configured_types_tags_and_milestone_roles() {
    let add = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["add", "--help"])
        .output()
        .expect("add help runs");
    let add = String::from_utf8(add.stdout).expect("add help is UTF-8");
    assert!(add.contains("Task type name declared in .tasks/config.json"));
    assert!(add.contains("Portable tag accepted by the project tag policy"));

    let milestone = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["milestone", "--help"])
        .output()
        .expect("milestone help runs");
    let milestone = String::from_utf8(milestone.stdout).expect("milestone help is UTF-8");
    assert!(milestone.contains("configured milestone-role task"));
    assert!(milestone.contains("configured criteria checklist"));
}
