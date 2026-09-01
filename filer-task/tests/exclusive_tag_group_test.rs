use std::fs;

use filer_task::{error::TaskError, project::TaskProject, validate::validate_repo};

#[test]
fn project_policy_exposes_configured_exclusive_tag_groups() {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
    fs::write(
        repo.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {"work": {"prefixes": ["WORK"]}},
  "task_types": {"Feature": {"criteria": "acceptance"}},
  "tags": {
    "policy": "strict",
    "allowed": ["bug", "enhancement", "needs-triage", "ready-for-agent"],
    "exclusive_groups": {
      "triage-category": ["bug", "enhancement"],
      "triage-state": ["needs-triage", "ready-for-agent"]
    }
  }
}"#,
    )
    .expect("configuration written");

    let project = TaskProject::open(repo.path()).expect("project opens");

    assert_eq!(
        project.policy().exclusive_tag_group("triage-state"),
        Some(&["needs-triage".to_string(), "ready-for-agent".to_string()][..])
    );
    assert_eq!(
        project.policy().exclusive_tag_group("triage-category"),
        Some(&["bug".to_string(), "enhancement".to_string()][..])
    );
}

#[test]
fn validation_rejects_multiple_tags_from_one_exclusive_group() {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir_all(repo.path().join(".tasks/work")).expect("task directory created");
    fs::write(
        repo.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {"work": {"prefixes": ["WORK"]}},
  "task_types": {"Feature": {"criteria": "acceptance"}},
  "tags": {
    "policy": "strict",
    "allowed": ["needs-triage", "ready-for-agent"],
    "exclusive_groups": {
      "triage-state": ["needs-triage", "ready-for-agent"]
    }
  }
}"#,
    )
    .expect("configuration written");
    fs::write(
        repo.path().join(".tasks/work/WORK-001-conflicting-triage.md"),
        "---\nid: WORK-001\ntitle: Conflicting triage task\nstatus: To Do\npriority: High\ntype: Feature\ntags: [needs-triage, ready-for-agent]\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("task written");
    let project = TaskProject::open(repo.path()).expect("project opens");

    let report = validate_repo(&project).expect("validation runs");

    assert_eq!(report.errors.len(), 1);
    assert!(report.errors[0].message.contains("triage-state"));
    assert!(report.errors[0].message.contains("needs-triage"));
    assert!(report.errors[0].message.contains("ready-for-agent"));
}

#[test]
fn project_rejects_exclusive_group_members_outside_the_allowed_catalog() {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
    fs::write(
        repo.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {"work": {"prefixes": ["WORK"]}},
  "task_types": {"Feature": {"criteria": "acceptance"}},
  "tags": {
    "policy": "strict",
    "allowed": ["ready-for-agent"],
    "exclusive_groups": {
      "triage-state": ["needs-triage", "ready-for-agent"]
    }
  }
}"#,
    )
    .expect("configuration written");

    let error = TaskProject::open(repo.path()).expect_err("unknown group member is rejected");

    assert!(matches!(error, TaskError::ConfigInvalidValue { .. }));
    assert!(error.to_string().contains("needs-triage"));
    assert!(error.to_string().contains("allowed tag catalog"));
}
