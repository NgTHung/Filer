use std::fs;

use filer_task::{
    error::TaskError, identity::TaskIdentity, lifecycle::set_criterion_checked,
    markdown::hashed_checklist_items, project::TaskProject,
};

fn project(content: &str) -> (tempfile::TempDir, TaskProject) {
    let temp = tempfile::tempdir().expect("temporary project created");
    fs::create_dir_all(temp.path().join(".tasks/core")).expect("domain created");
    fs::write(task_path(&temp), content).expect("task written");
    let project = TaskProject::open(temp.path()).expect("project opens");
    (temp, project)
}

fn task_path(temp: &tempfile::TempDir) -> std::path::PathBuf {
    temp.path().join(".tasks/core/CORE-001-task.md")
}

fn identity() -> TaskIdentity {
    TaskIdentity::new("core", "CORE-001").expect("valid identity")
}

fn content(criteria: &str) -> String {
    format!(
        "---\nid: CORE-001\ntitle: Conditional criteria\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n{criteria}"
    )
}

#[test]
fn checklist_hashes_include_the_exact_source_line() {
    let content = content("- [ ] Works\r\n- [x] Works\r\n  - [ ] Works  \r\n");

    let items = hashed_checklist_items(&content, "Acceptance Criteria");

    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0].content_hash,
        "59ac2307d5d7d305aa318475ebdc4cdcaa4a97b0a7f3ad8715c86d953d6f4d24"
    );
    assert_eq!(
        items[1].content_hash,
        "e477d7cf228b62f1ce8dbef4e1afa234e2eaadaa4280719d41f549226c23345e"
    );
    assert_eq!(
        items[2].content_hash,
        "7abb4a4a254aa4e86ea6a216d4bddcc1c435c09c5237282406b5ee06162190a2"
    );
}

#[test]
fn conditional_set_changes_only_the_requested_marker_and_returns_a_new_hash() {
    let original = content("- [ ] First  \r\n  - [x] Second\r\n");
    let (temp, project) = project(&original);
    let items = hashed_checklist_items(&original, "Acceptance Criteria");

    set_criterion_checked(&project, &identity(), 0, &items[0].content_hash, true)
        .expect("conditional update succeeds");

    let updated = fs::read_to_string(task_path(&temp)).expect("task readable");
    assert_eq!(changed_byte_count(&original, &updated), 1);
    assert!(updated.contains("- [x] First  \r\n  - [x] Second"));
    let updated_items = hashed_checklist_items(&updated, "Acceptance Criteria");
    assert_ne!(updated_items[0].content_hash, items[0].content_hash);
    assert_eq!(updated_items[1].content_hash, items[1].content_hash);
}

#[test]
fn conditional_set_skips_writing_an_already_requested_state() {
    let original = content("- [ ] Works\n");
    let (temp, project) = project(&original);
    let item = hashed_checklist_items(&original, "Acceptance Criteria")
        .into_iter()
        .next()
        .expect("criterion exists");
    let path = task_path(&temp);
    let original_permissions = fs::metadata(&path)
        .expect("metadata readable")
        .permissions();
    let mut read_only = original_permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).expect("task made read-only");

    let result = set_criterion_checked(&project, &identity(), 0, &item.content_hash, false);

    fs::set_permissions(&path, original_permissions).expect("permissions restored");
    assert!(result.is_ok(), "unchanged state must not replace the file");
    assert_eq!(fs::read_to_string(path).expect("task readable"), original);
}

#[test]
fn conditional_set_rejects_changed_or_reordered_content() {
    let original = content("- [ ] First\n- [ ] Second\n");
    let (temp, project) = project(&original);
    let stale_hash = hashed_checklist_items(&original, "Acceptance Criteria")[0]
        .content_hash
        .clone();
    let reordered = content("- [ ] Second\n- [ ] First\n");
    fs::write(task_path(&temp), &reordered).expect("criteria reordered");
    let project = project.reload().expect("project reloads");

    let error = set_criterion_checked(&project, &identity(), 0, &stale_hash, true)
        .expect_err("stale content is rejected");

    assert!(matches!(
        error,
        TaskError::CriterionContentMismatch { index: 0, .. }
    ));
    assert_eq!(error.code(), "criterion_content_mismatch");
    assert_eq!(error.context()["expected_hash"], stale_hash);
    assert_eq!(
        error.context()["actual_hash"],
        hashed_checklist_items(&reordered, "Acceptance Criteria")[0].content_hash
    );
    assert_eq!(
        fs::read_to_string(task_path(&temp)).expect("task readable"),
        reordered
    );
}

#[test]
fn conditional_set_rejects_an_out_of_range_index() {
    let original = content("- [ ] Works\n");
    let (_temp, project) = project(&original);

    let error = set_criterion_checked(&project, &identity(), 1, &"0".repeat(64), true)
        .expect_err("invalid index is rejected");

    assert!(matches!(
        error,
        TaskError::CriterionIndexOutOfRange {
            index: 1,
            count: 1,
            ..
        }
    ));
}

fn changed_byte_count(left: &str, right: &str) -> usize {
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .filter(|(left, right)| left != right)
        .count()
        + left.len().abs_diff(right.len())
}
