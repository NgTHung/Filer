use std::fs;

use taskroot::{
    error::TaskError,
    frontmatter::parse_metadata,
    identity::TaskIdentity,
    lifecycle::{Criterion, FieldPatch, NewTask, TaskPatch, add_task, edit_task},
    model::{Priority, Risk, TaskStatus, TaskType},
    project::TaskProject,
};

fn project() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temporary project created");
    fs::create_dir_all(temp.path().join(".tasks/core")).expect("domain created");
    temp
}

fn identity(id: &str) -> TaskIdentity {
    TaskIdentity::new("core", id).expect("valid identity")
}

#[test]
fn edit_task_applies_multi_field_patch() {
    let temp = project();
    write_task(temp.path(), "CORE-001", "Parent task", "");
    write_task(temp.path(), "CORE-002", "Editable task", "");
    let project = TaskProject::open(temp.path()).expect("project opens");

    let path = edit_task(
        &project,
        &identity("CORE-002"),
        TaskPatch {
            title: Some("Edited task title".to_string()),
            summary: Some("Updated summary text.".to_string()),
            risk: FieldPatch::Set(Risk::High),
            impact: FieldPatch::Set("Touches task editing behavior.".to_string()),
            tags: Some(vec!["editing".to_string(), "library".to_string()]),
            parent: FieldPatch::Set("CORE-001".to_string()),
            depends_on: Some(vec!["CORE-001".to_string()]),
            ..TaskPatch::default()
        },
    )
    .expect("edit succeeds");

    let content = fs::read_to_string(path).expect("task readable");
    assert!(content.contains("title: \"Edited task title\""));
    assert!(content.contains("parent: \"CORE-001\""));
    assert!(content.contains("depends_on: [\"CORE-001\"]"));
    assert!(content.contains("risk: \"High\""));
    assert!(content.contains("impact: \"Touches task editing behavior.\""));
    assert!(content.contains("tags: [\"editing\", \"library\"]"));
    assert!(content.contains("## Summary\n\nUpdated summary text."));
}

#[test]
fn edit_task_rejects_dependency_cycle_without_writing() {
    let temp = project();
    let first = write_task(temp.path(), "CORE-001", "First task", "");
    write_task(
        temp.path(),
        "CORE-002",
        "Second task",
        "depends_on: [CORE-001]\n",
    );
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original = fs::read_to_string(&first).expect("task readable");

    let error = edit_task(
        &project,
        &identity("CORE-001"),
        TaskPatch {
            depends_on: Some(vec!["CORE-002".to_string()]),
            ..TaskPatch::default()
        },
    )
    .unwrap_err();

    assert!(matches!(error, TaskError::Validation(_)), "{error:?}");
    assert!(error.to_string().contains("dependency cycle detected"));
    assert_eq!(fs::read_to_string(first).unwrap(), original);
}

#[test]
fn edit_task_rejects_unknown_milestone_without_writing() {
    let temp = project();
    let path = write_task(temp.path(), "CORE-001", "Editable task", "");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original = fs::read_to_string(&path).expect("task readable");

    let error = edit_task(
        &project,
        &identity("CORE-001"),
        TaskPatch {
            milestone: FieldPatch::Set("0.4.0".to_string()),
            ..TaskPatch::default()
        },
    )
    .unwrap_err();

    assert!(matches!(error, TaskError::Validation(_)), "{error:?}");
    assert!(error.to_string().contains("milestone 0.4.0"));
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn edit_task_empty_patch_preserves_file_bytes() {
    let temp = project();
    let path = write_task(temp.path(), "CORE-001", "Editable task", "");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original = fs::read(&path).expect("task readable");

    edit_task(&project, &identity("CORE-001"), TaskPatch::default()).expect("no-op succeeds");

    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn create_and_edit_round_trip_yaml_sensitive_titles() {
    let temp = project();
    let project = TaskProject::open(temp.path()).expect("project opens");
    let created_title = "- lead # value: \"quoted\" \\ path\nsecond line";
    let path = add_task(
        &project,
        NewTask {
            domain: "core".to_string(),
            id: "CORE-001".to_string(),
            title: created_title.to_string(),
            status: TaskStatus::ToDo,
            priority: Priority::High,
            task_type: TaskType::new("Feature"),
            parent: None,
            milestone: None,
            depends_on: Vec::new(),
            rules: Vec::new(),
            risk: None,
            impact: Some("Impact # value: \"quoted\" \\ path\nsecond line".to_string()),
            tags: Vec::new(),
            whitepaper: None,
            summary: Some("Summary.".to_string()),
            criteria: vec![Criterion {
                text: "Works".to_string(),
                checked: false,
            }],
            rationale: None,
            blocked_reason: None,
        },
    )
    .expect("task creation succeeds");

    let created = fs::read_to_string(&path).expect("task readable");
    let metadata = parse_metadata(&path, &created).expect("created frontmatter parses");
    assert_eq!(metadata.title, created_title);
    assert_eq!(
        metadata.impact.as_deref(),
        Some("Impact # value: \"quoted\" \\ path\nsecond line")
    );

    let edited_title = "Edited # title: - \"quoted\" \\ path\nnext line";
    let project = project.reload().expect("project reloads");
    edit_task(
        &project,
        &identity("CORE-001"),
        TaskPatch {
            title: Some(edited_title.to_string()),
            ..TaskPatch::default()
        },
    )
    .expect("task edit succeeds");

    let edited = fs::read_to_string(&path).expect("task readable");
    let metadata = parse_metadata(&path, &edited).expect("edited frontmatter parses");
    assert_eq!(metadata.title, edited_title);
}

#[test]
fn local_edits_skip_unrelated_files_but_relationship_edits_validate_the_repository() {
    let temp = project();
    let path = write_task(temp.path(), "CORE-001", "Editable task", "");
    fs::write(
        temp.path().join(".tasks/core/CORE-002-invalid.md"),
        "---\nid: [\n---\n",
    )
    .expect("invalid sibling written");
    let project = TaskProject::open(temp.path()).expect("project opens");

    edit_task(
        &project,
        &identity("CORE-001"),
        TaskPatch {
            title: Some("Locally edited task".to_string()),
            ..TaskPatch::default()
        },
    )
    .expect("local edit validates only its target");

    let before_relationship = fs::read(&path).expect("task readable");
    let error = edit_task(
        &project,
        &identity("CORE-001"),
        TaskPatch {
            depends_on: Some(Vec::new()),
            ..TaskPatch::default()
        },
    )
    .expect_err("relationship edit validates the repository");

    assert!(matches!(error, TaskError::Validation(_)), "{error:?}");
    assert_eq!(fs::read(path).expect("task readable"), before_relationship);
}

fn write_task(root: &std::path::Path, id: &str, title: &str, extra: &str) -> std::path::PathBuf {
    let path = root.join(".tasks/core").join(format!(
        "{id}-{}.md",
        title.to_ascii_lowercase().replace(' ', "-")
    ));
    fs::write(
        &path,
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n{extra}---\n\n## Summary\n\nOriginal summary.\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task written");
    path
}
