use std::{fs, process::Command};

use filer_task::{
    error::TaskError, frontmatter::parse_metadata, identity::TaskIdentity,
    lifecycle::set_exclusive_tag_group_value, project::TaskProject, validate::validate_repo,
};
use serde_json::Value;

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
fn list_and_ready_filters_combine_triage_tags_with_structural_readiness() {
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
        repo.path().join(".tasks/work/WORK-001-ready.md"),
        "---\nid: WORK-001\ntitle: Ready triage task\nstatus: To Do\npriority: High\ntype: Feature\ntags: [ready-for-agent]\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("ready task written");
    fs::write(
        repo.path().join(".tasks/work/WORK-002-blocked.md"),
        "---\nid: WORK-002\ntitle: Blocked triage task\nstatus: To Do\npriority: High\ntype: Feature\ndepends_on: [WORK-001]\ntags: [ready-for-agent]\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("blocked task written");
    let root = repo.path().to_str().expect("temporary path is UTF-8");

    let listed = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "list",
            "--root",
            root,
            "--tag",
            "ready-for-agent",
            "--format",
            "json",
        ])
        .output()
        .expect("list command runs");
    assert!(listed.status.success());
    let listed: Value = serde_json::from_slice(&listed.stdout).expect("list output is JSON");
    assert_eq!(listed.as_array().map(Vec::len), Some(2));

    let ready = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "ready",
            "--root",
            root,
            "--tag",
            "ready-for-agent",
            "--format",
            "json",
        ])
        .output()
        .expect("ready command runs");
    assert!(ready.status.success());
    let ready: Value = serde_json::from_slice(&ready.stdout).expect("ready output is JSON");
    assert_eq!(ready["tasks"].as_array().map(Vec::len), Some(1));
    assert_eq!(ready["tasks"][0]["qualified_id"], "work:WORK-001");
}

#[test]
fn cli_sets_an_exclusive_tag_group_value() {
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
    "allowed": ["workflow", "needs-triage", "ready-for-agent"],
    "exclusive_groups": {
      "triage-state": ["needs-triage", "ready-for-agent"]
    }
  }
}"#,
    )
    .expect("configuration written");
    let task_path = repo.path().join(".tasks/work/WORK-001-triage.md");
    fs::write(
        &task_path,
        "---\nid: WORK-001\ntitle: Triage workflow task\nstatus: To Do\npriority: High\ntype: Feature\ntags: [workflow, needs-triage]\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("task written");

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["tag", "set", "--root"])
        .arg(repo.path())
        .args(["work:WORK-001", "triage-state", "ready-for-agent"])
        .output()
        .expect("command runs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Task Tags Updated"));
    let content = fs::read_to_string(&task_path).expect("updated task is readable");
    let metadata = parse_metadata(&task_path, &content).expect("updated task parses");
    assert_eq!(metadata.tags, ["workflow", "ready-for-agent"]);

    let updated = fs::read(&task_path).expect("updated bytes are readable");
    let rejected = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["tag", "set", "--root"])
        .arg(repo.path())
        .args(["work:WORK-001", "triage-state", "ready-for-human"])
        .output()
        .expect("rejected command runs");
    assert!(!rejected.status.success());
    assert_eq!(
        fs::read(&task_path).expect("rejected task remains readable"),
        updated
    );

    let cleared = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["tag", "clear", "--root"])
        .arg(repo.path())
        .args(["work:WORK-001", "triage-state"])
        .output()
        .expect("clear command runs");
    assert!(
        cleared.status.success(),
        "{}",
        String::from_utf8_lossy(&cleared.stderr)
    );
    let content = fs::read_to_string(&task_path).expect("cleared task is readable");
    let metadata = parse_metadata(&task_path, &content).expect("cleared task parses");
    assert_eq!(metadata.tags, ["workflow"]);
}

#[test]
fn library_sets_and_clears_one_exclusive_tag_group_value() {
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
    "allowed": ["workflow", "needs-triage", "ready-for-agent"],
    "exclusive_groups": {
      "triage-state": ["needs-triage", "ready-for-agent"]
    }
  }
}"#,
    )
    .expect("configuration written");
    let task_path = repo.path().join(".tasks/work/WORK-001-triage.md");
    fs::write(
        &task_path,
        "---\nid: WORK-001\ntitle: Triage workflow task\nstatus: To Do\npriority: High\ntype: Feature\ntags: [workflow, needs-triage]\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("task written");
    let project = TaskProject::open(repo.path()).expect("project opens");
    let identity = TaskIdentity::new("work", "WORK-001").expect("identity is valid");

    set_exclusive_tag_group_value(&project, &identity, "triage-state", Some("ready-for-agent"))
        .expect("group value is set");
    let content = fs::read_to_string(&task_path).expect("updated task is readable");
    let metadata = parse_metadata(&task_path, &content).expect("updated task parses");
    assert_eq!(metadata.tags, ["workflow", "ready-for-agent"]);

    set_exclusive_tag_group_value(&project, &identity, "triage-state", None)
        .expect("group value is cleared");
    let content = fs::read_to_string(&task_path).expect("cleared task is readable");
    let metadata = parse_metadata(&task_path, &content).expect("cleared task parses");
    assert_eq!(metadata.tags, ["workflow"]);
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

#[test]
fn policy_mutation_cannot_remove_a_tag_used_by_an_exclusive_group() {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
    let config_path = repo.path().join(".tasks/config.json");
    fs::write(
        &config_path,
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
    let original = fs::read(&config_path).expect("configuration is readable");
    let project = TaskProject::open(repo.path()).expect("project opens");

    project
        .remove_tag("ready-for-agent")
        .expect_err("group member cannot be removed from the catalog");

    assert_eq!(
        fs::read(&config_path).expect("configuration remains readable"),
        original
    );
}
