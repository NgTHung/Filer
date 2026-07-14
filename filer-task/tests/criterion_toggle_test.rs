use std::{fs, process::Command};

use filer_task::{
    error::TaskError, identity::TaskIdentity, lifecycle::toggle_criterion,
    markdown::checklist_items, project::TaskProject,
};

fn project() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temporary project created");
    fs::create_dir_all(temp.path().join(".tasks/core")).expect("domain created");
    fs::write(
        temp.path().join(".tasks/core/CORE-001-toggle-task.md"),
        "---\nid: CORE-001\ntitle: Toggle task\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Summary\n\nOriginal summary.\n\n## Acceptance Criteria\n\n- [ ] First item\n- [x] Second item\n",
    )
    .expect("task written");
    temp
}

fn identity() -> TaskIdentity {
    TaskIdentity::new("core", "CORE-001").expect("valid identity")
}

#[test]
fn toggle_criterion_flips_first_item_only() {
    let temp = project();
    let task = temp.path().join(".tasks/core/CORE-001-toggle-task.md");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original = fs::read_to_string(&task).expect("task readable");

    toggle_criterion(&project, &identity(), 0).expect("toggle succeeds");

    let updated = fs::read_to_string(task).expect("task readable");
    assert_eq!(changed_byte_count(&original, &updated), 1);
    assert!(updated.contains("- [x] First item\n- [x] Second item"));
}

#[test]
fn toggle_criterion_flips_last_item_and_retoggles_to_original() {
    let temp = project();
    let task = temp.path().join(".tasks/core/CORE-001-toggle-task.md");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original = fs::read(&task).expect("task readable");

    toggle_criterion(&project, &identity(), 1).expect("toggle succeeds");
    let updated = fs::read_to_string(&task).expect("task readable");
    assert!(updated.contains("- [ ] First item\n- [ ] Second item"));

    let project = project.reload().expect("project reloads");
    toggle_criterion(&project, &identity(), 1).expect("toggle succeeds");

    assert_eq!(fs::read(task).unwrap(), original);
}

#[test]
fn toggle_criterion_rejects_out_of_range_index() {
    let temp = project();
    let project = TaskProject::open(temp.path()).expect("project opens");

    let error = toggle_criterion(&project, &identity(), 2).unwrap_err();

    assert!(matches!(
        error,
        TaskError::CriterionIndexOutOfRange {
            index: 2,
            count: 2,
            ..
        }
    ));
    assert_eq!(error.code(), "criterion_index_out_of_range");
}

#[test]
fn criterion_toggle_command_uses_library_function() {
    let temp = project();

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args([
            "criterion-toggle",
            "--root",
            temp.path().to_str().expect("temp path should be UTF-8"),
            "core:CORE-001",
            "0",
        ])
        .output()
        .expect("command runs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "Task Criterion Toggled\nTask: core:CORE-001\nPath: .tasks/core/CORE-001-toggle-task.md\n"
    );
}

#[test]
fn toggle_criterion_uses_the_same_marker_order_as_checklist_items() {
    let temp = project();
    let task = temp.path().join(".tasks/core/CORE-001-toggle-task.md");
    let content = "---\r\nid: CORE-001\r\ntitle: Toggle task\r\nstatus: To Do\r\npriority: High\r\ntype: Feature\r\n---\r\n\r\n## Acceptance Criteria\r\n\r\n  - [ ] Indented\r\n- [x] Lowercase\r\n    - [X] Uppercase\r\n- [-] Malformed\r\n- [] Malformed empty\r\n";
    fs::write(&task, content).expect("task written");
    let items = checklist_items(content, "Acceptance Criteria");

    assert_eq!(items.len(), 3);
    assert_eq!(
        items.iter().map(|item| item.checked).collect::<Vec<_>>(),
        [false, true, true]
    );

    for index in 0..items.len() {
        fs::write(&task, content).expect("task reset");
        let project = TaskProject::open(temp.path()).expect("project opens");
        toggle_criterion(&project, &identity(), index).expect("toggle succeeds");
        let updated = fs::read_to_string(&task).expect("task readable");

        assert_eq!(changed_byte_count(content, &updated), 1);
        assert_eq!(
            updated
                .as_bytes()
                .iter()
                .filter(|byte| **byte == b'\r')
                .count(),
            15
        );
        let updated_items = checklist_items(&updated, "Acceptance Criteria");
        for (item_index, item) in updated_items.iter().enumerate() {
            assert_eq!(
                item.checked,
                items[item_index].checked ^ (item_index == index)
            );
        }
    }
}

fn changed_byte_count(left: &str, right: &str) -> usize {
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .filter(|(left, right)| left != right)
        .count()
        + left.len().abs_diff(right.len())
}
