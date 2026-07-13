use std::{fs, process::Command};

use filer_task::{
    error::TaskError,
    project::{CriteriaPolicy, TagPolicy, TaskProject},
};

fn project() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temp project created");
    fs::create_dir(temp.path().join(".tasks")).expect("task directory created");
    temp
}

fn write_config(root: &std::path::Path, config: &str) {
    fs::write(root.join(".tasks/config.json"), config).expect("configuration written");
}

fn minimal_config() -> &'static str {
    r#"{
        "version": 1,
        "domains": {"work": {"prefixes": ["WORK"]}},
        "task_types": {"Feature": {"criteria": "acceptance"}},
        "tags": {"policy": "open"}
    }"#
}

#[test]
fn opens_valid_custom_configuration() {
    let temp = project();
    write_config(temp.path(), minimal_config());

    let project = TaskProject::open(temp.path()).expect("project opens");

    assert_eq!(project.root(), temp.path().canonicalize().unwrap());
    assert!(!project.policy().is_compatibility());
    assert_eq!(project.policy().domains().len(), 1);
    assert_eq!(
        project.policy().domain("work").unwrap().prefixes(),
        &["WORK"]
    );
    assert_eq!(
        project.policy().task_type("Feature").unwrap().criteria(),
        CriteriaPolicy::Acceptance
    );
    assert_eq!(project.policy().milestone_type(), None);
    assert_eq!(project.policy().tags(), &TagPolicy::Open);
}

#[test]
fn absent_configuration_uses_filer_compatibility_policy() {
    let temp = project();

    let project = TaskProject::open(temp.path()).expect("project opens");
    let policy = project.policy();

    assert!(policy.is_compatibility());
    assert!(policy.domain("core").unwrap().allows_prefix("UTILS"));
    assert!(policy.domain("app").unwrap().allows_prefix("UI"));
    assert!(policy.domain("ecosystem").unwrap().allows_prefix("PLUG"));
    assert!(
        policy
            .domain("milestones")
            .unwrap()
            .allows_prefix("MILESTONE")
    );
    assert_eq!(policy.milestone_type(), Some("Milestone"));
    assert_eq!(policy.tags(), &TagPolicy::Open);
}

#[test]
fn independently_opened_projects_keep_separate_policies() {
    let first = project();
    let second = project();
    write_config(first.path(), &minimal_config().replace("WORK", "ONE"));
    write_config(second.path(), &minimal_config().replace("WORK", "TWO"));

    let first = TaskProject::open(first.path()).expect("first opens");
    let second = TaskProject::open(second.path()).expect("second opens");

    assert!(first.policy().domain("work").unwrap().allows_prefix("ONE"));
    assert!(!first.policy().domain("work").unwrap().allows_prefix("TWO"));
    assert!(second.policy().domain("work").unwrap().allows_prefix("TWO"));
}

#[test]
fn rejects_invalid_json_with_parser_location() {
    let temp = project();
    write_config(temp.path(), "{\n  \"version\": 1,");

    let error = TaskProject::open(temp.path()).unwrap_err();

    assert!(matches!(error, TaskError::ConfigInvalidJson { line: 2, column, .. } if column > 0));
    assert_eq!(error.code(), "config_invalid_json");
}

#[test]
fn rejects_unsupported_version() {
    let temp = project();
    write_config(
        temp.path(),
        &minimal_config().replace("\"version\": 1", "\"version\": 2"),
    );

    let error = TaskProject::open(temp.path()).unwrap_err();

    assert!(matches!(
        error,
        TaskError::ConfigUnsupportedVersion {
            received: 2,
            supported: 1,
            ..
        }
    ));
}

#[test]
fn rejects_unknown_fields_at_every_level() {
    for (path, config) in [
        (
            "$",
            minimal_config().replace("\"version\": 1,", "\"version\": 1, \"extra\": true,"),
        ),
        (
            "$.domains.work",
            minimal_config().replace(
                "\"prefixes\": [\"WORK\"]",
                "\"prefixes\": [\"WORK\"], \"extra\": true",
            ),
        ),
        (
            "$.task_types.Feature",
            minimal_config().replace(
                "\"criteria\": \"acceptance\"",
                "\"criteria\": \"acceptance\", \"extra\": true",
            ),
        ),
        (
            "$.tags",
            minimal_config().replace(
                "\"policy\": \"open\"",
                "\"policy\": \"open\", \"extra\": true",
            ),
        ),
    ] {
        let temp = project();
        write_config(temp.path(), &config);
        let error = TaskProject::open(temp.path()).unwrap_err();
        assert!(
            matches!(error, TaskError::ConfigUnknownField { path: actual, field, .. } if actual == path && field == "extra")
        );
    }
}

#[test]
fn rejects_duplicate_object_keys_and_array_values() {
    let cases = [
        ("$.domains", "work", minimal_config().replace(
            "\"domains\": {\"work\": {\"prefixes\": [\"WORK\"]}}",
            "\"domains\": {\"work\": {\"prefixes\": [\"WORK\"]}, \"work\": {\"prefixes\": [\"ALT\"]}}",
        )),
        ("$.domains.work.prefixes", "WORK", minimal_config().replace("[\"WORK\"]", "[\"WORK\", \"WORK\"]")),
        ("$.tags.allowed", "backend", minimal_config().replace(
            "{\"policy\": \"open\"}",
            "{\"policy\": \"strict\", \"allowed\": [\"backend\", \"backend\"]}",
        )),
    ];

    for (path, value, config) in cases {
        let temp = project();
        write_config(temp.path(), &config);
        let error = TaskProject::open(temp.path()).unwrap_err();
        assert!(
            matches!(error, TaskError::ConfigDuplicate { path: actual, value: actual_value, .. } if actual == path && actual_value == value)
        );
    }
}

#[test]
fn rejects_missing_null_and_empty_required_values() {
    let cases = [
        minimal_config().replace("\"domains\": {\"work\": {\"prefixes\": [\"WORK\"]}},", ""),
        minimal_config().replace("{\"work\": {\"prefixes\": [\"WORK\"]}}", "{}"),
        minimal_config().replace("[\"WORK\"]", "[]"),
        minimal_config().replace(
            "\"task_types\": {\"Feature\": {\"criteria\": \"acceptance\"}}",
            "\"task_types\": {}",
        ),
        minimal_config().replace("{\"policy\": \"open\"}", "null"),
    ];

    for config in cases {
        let temp = project();
        write_config(temp.path(), &config);
        assert!(matches!(
            TaskProject::open(temp.path()),
            Err(TaskError::ConfigInvalidValue { .. })
        ));
    }
}

#[test]
fn rejects_invalid_portable_names_and_windows_devices() {
    for (needle, replacement) in [
        ("\"work\"", "\"Core\""),
        ("\"work\"", "\"con\""),
        ("\"WORK\"", "\"1WORK\""),
        ("\"WORK\"", "\"COM1\""),
        ("\"Feature\"", "\"feature\""),
    ] {
        let temp = project();
        write_config(temp.path(), &minimal_config().replace(needle, replacement));
        assert!(matches!(
            TaskProject::open(temp.path()),
            Err(TaskError::ConfigInvalidValue { .. })
        ));
    }

    let temp = project();
    let config = minimal_config().replace(
        "{\"policy\": \"open\"}",
        "{\"policy\": \"strict\", \"allowed\": [\"Bad-Tag\"]}",
    );
    write_config(temp.path(), &config);
    assert!(matches!(
        TaskProject::open(temp.path()),
        Err(TaskError::ConfigInvalidValue { .. })
    ));
}

#[test]
fn rejects_conflicting_milestone_roles() {
    let temp = project();
    let config = minimal_config().replace(
        "\"Feature\": {\"criteria\": \"acceptance\"}",
        "\"Feature\": {\"criteria\": \"acceptance\", \"role\": \"milestone\"}, \"Gate\": {\"criteria\": \"exit\", \"role\": \"milestone\"}",
    );
    write_config(temp.path(), &config);

    let error = TaskProject::open(temp.path()).unwrap_err();
    assert!(
        matches!(error, TaskError::ConfigInvalidValue { path, value, .. } if path == "$.task_types" && value.contains("Feature") && value.contains("Gate"))
    );
}

#[test]
fn validates_open_and_strict_tag_policy_shapes() {
    let invalid = [
        minimal_config().replace(
            "{\"policy\": \"open\"}",
            "{\"policy\": \"open\", \"allowed\": []}",
        ),
        minimal_config().replace("{\"policy\": \"open\"}", "{\"policy\": \"strict\"}"),
        minimal_config().replace("{\"policy\": \"open\"}", "{\"policy\": \"closed\"}"),
    ];
    for config in invalid {
        let temp = project();
        write_config(temp.path(), &config);
        assert!(matches!(
            TaskProject::open(temp.path()),
            Err(TaskError::ConfigInvalidValue { .. })
        ));
    }

    let temp = project();
    write_config(
        temp.path(),
        &minimal_config().replace(
            "{\"policy\": \"open\"}",
            "{\"policy\": \"strict\", \"allowed\": []}",
        ),
    );
    let project = TaskProject::open(temp.path()).expect("empty strict policy is valid");
    assert_eq!(
        project.policy().tags(),
        &TagPolicy::Strict {
            allowed: Vec::new()
        }
    );
}

#[test]
fn reports_configuration_filesystem_errors() {
    let temp = project();
    fs::create_dir(temp.path().join(".tasks/config.json")).expect("directory created");

    let error = TaskProject::open(temp.path()).unwrap_err();

    assert!(matches!(
        error,
        TaskError::ConfigIo {
            operation: "read",
            ..
        }
    ));
    assert_eq!(error.code(), "config_io");
}

#[test]
fn cli_reports_configuration_before_reading_tasks() {
    let temp = project();
    fs::create_dir(temp.path().join(".tasks/core")).expect("domain created");
    fs::write(
        temp.path().join(".tasks/core/CORE-001-invalid.md"),
        "not a task",
    )
    .expect("invalid task written");
    write_config(temp.path(), "{ invalid");

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["validate", "--root"])
        .arg(temp.path())
        .output()
        .expect("command runs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains("invalid task configuration JSON"),
        "{stderr}"
    );
    assert!(!stderr.contains("task validation failed"), "{stderr}");
}

#[test]
fn malformed_configuration_prevents_lifecycle_writes() {
    let temp = project();
    let domain = temp.path().join(".tasks/core");
    fs::create_dir(&domain).expect("domain created");
    let task_path = domain.join("CORE-001-task.md");
    let original = "---\nid: CORE-001\ntitle: Existing task\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n";
    fs::write(&task_path, original).expect("task written");
    write_config(temp.path(), "{}");

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["start", "CORE-001", "--root"])
        .arg(temp.path())
        .output()
        .expect("command runs");

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(task_path).unwrap(), original);
}
