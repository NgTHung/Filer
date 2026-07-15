use std::{fs, path::Path};

use filer_task::{
    milestone::{build_milestone_aggregations, tasks_for_milestone},
    project::TaskProject,
    validate::{require_valid_report, validate_repo},
};

#[test]
fn aggregation_is_empty_without_milestone_declarations() {
    let repo = project();
    let project = TaskProject::open(repo.path()).expect("project opens");

    let milestones = build_milestone_aggregations(&project, &[]).expect("aggregation builds");

    assert!(milestones.is_empty());
}

#[test]
fn aggregation_uses_configured_role_and_orders_milestones_and_tasks() {
    let repo = project();
    write_task(
        repo.path(),
        "release",
        "REL-002",
        "Second release",
        "To Do",
        "ReleaseGate",
        "milestone: \"2.0.0\"\n",
        false,
    );
    write_task(
        repo.path(),
        "release",
        "REL-001",
        "First release",
        "In Progress",
        "ReleaseGate",
        "milestone: \"1.0.0\"\n",
        false,
    );
    write_task(
        repo.path(),
        "backend",
        "WORK-003",
        "Blocked work",
        "Blocked",
        "Change",
        "milestone: \"1.0.0\"\n",
        false,
    );
    write_task(
        repo.path(),
        "backend",
        "WORK-002",
        "Finished work",
        "Done",
        "Change",
        "milestone: \"1.0.0\"\n",
        true,
    );
    write_task(
        repo.path(),
        "backend",
        "WORK-001",
        "Ready work",
        "To Do",
        "Change",
        "milestone: \"1.0.0\"\n",
        false,
    );
    let project = TaskProject::open(repo.path()).expect("project opens");
    let tasks = require_valid_report(validate_repo(&project).expect("repository scans"))
        .expect("repository validates");

    let milestones = build_milestone_aggregations(&project, &tasks).expect("aggregation builds");

    assert_eq!(milestones.len(), 2);
    assert_eq!(milestones[0].milestone.qualified_id(), "release:REL-001");
    assert_eq!(milestones[0].criteria_heading, "Acceptance Criteria");
    assert_eq!(milestones[0].criteria.len(), 1);
    assert_eq!(milestones[0].criteria[0].text, "Works");
    assert_eq!(milestones[0].done, 1);
    assert_eq!(milestones[0].total, 4);
    assert_eq!(
        milestones[0].tasks_by_status["To Do"]
            .iter()
            .map(|task| task.qualified_id())
            .collect::<Vec<_>>(),
        ["backend:WORK-001"]
    );
    assert_eq!(
        milestones[0].tasks_by_status["In Progress"][0].qualified_id(),
        "release:REL-001"
    );
    assert_eq!(
        milestones[0].tasks_by_status["Blocked"][0].qualified_id(),
        "backend:WORK-003"
    );
    assert_eq!(
        milestones[0].tasks_by_status["Done"][0].qualified_id(),
        "backend:WORK-002"
    );
    assert_eq!(milestones[1].milestone.qualified_id(), "release:REL-002");
    assert_eq!(milestones[1].done, 0);
    assert_eq!(milestones[1].total, 1);

    let scoped = tasks_for_milestone(&tasks, "1.0.0")
        .map(|task| task.qualified_id())
        .collect::<Vec<_>>();
    assert!(scoped.contains(&"release:REL-001".to_string()));
}

fn project() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temp project created");
    fs::create_dir(temp.path().join(".tasks")).expect("task directory created");
    fs::write(
        temp.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {
    "backend": {"prefixes": ["WORK"]},
    "release": {"prefixes": ["REL"]}
  },
  "task_types": {
    "Change": {"criteria": "acceptance"},
    "ReleaseGate": {"criteria": "acceptance", "role": "milestone"}
  },
  "tags": {"policy": "open"}
}"#,
    )
    .expect("configuration written");
    temp
}

#[allow(clippy::too_many_arguments)]
fn write_task(
    root: &Path,
    domain: &str,
    id: &str,
    title: &str,
    status: &str,
    task_type: &str,
    extra: &str,
    checked: bool,
) {
    let directory = root.join(".tasks").join(domain);
    fs::create_dir_all(&directory).expect("domain directory created");
    let marker = if checked { "x" } else { " " };
    let blocked_reason = if status == "Blocked" {
        "\n## Blocked Reason\n\nWaiting for input.\n"
    } else {
        ""
    };
    fs::write(
        directory.join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: High\ntype: {task_type}\n{extra}---\n\n## Summary\n\nTest task.\n\n## Acceptance Criteria\n\n- [{marker}] Works\n{blocked_reason}"
        ),
    )
    .expect("task written");
}
