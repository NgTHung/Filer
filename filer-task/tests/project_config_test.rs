use std::{fs, process::Command};

use filer_task::{
    error::TaskError,
    project::{CriteriaPolicy, InitDomain, InitProjectOptions, TagPolicy, TaskProject},
    validate::{require_valid_report, validate_repo},
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
fn initializes_project_without_domain_directories() {
    let temp = tempfile::tempdir().expect("temp project created");

    let project = TaskProject::init(
        temp.path(),
        InitProjectOptions {
            domain: InitDomain::new("work", ["WORK"]),
        },
    )
    .expect("project initializes");

    assert_eq!(project.root(), temp.path().canonicalize().unwrap());
    assert!(temp.path().join(".tasks/config.json").is_file());
    assert!(!temp.path().join(".tasks/work").exists());
    assert!(
        project
            .policy()
            .domain("work")
            .unwrap()
            .allows_prefix("WORK")
    );
    let validated = require_valid_report(validate_repo(&project).expect("project validates"))
        .expect("empty project should be valid");
    assert!(validated.tasks.is_empty());
}

#[test]
fn init_rejects_invalid_options_without_creating_tasks_and_allows_retry() {
    let cases = [
        InitDomain {
            name: "invalid/domain".to_string(),
            prefixes: vec!["WORK".to_string()],
        },
        InitDomain {
            name: "work".to_string(),
            prefixes: Vec::new(),
        },
        InitDomain {
            name: "work".to_string(),
            prefixes: vec!["invalid-prefix".to_string()],
        },
        InitDomain {
            name: "work".to_string(),
            prefixes: vec!["WORK".to_string(), "WORK".to_string()],
        },
    ];

    for domain in cases {
        let temp = tempfile::tempdir().expect("temp project created");

        TaskProject::init(temp.path(), InitProjectOptions { domain })
            .expect_err("invalid initialization must fail");

        assert!(!temp.path().join(".tasks").exists());
        TaskProject::init(
            temp.path(),
            InitProjectOptions {
                domain: InitDomain::new("work", ["WORK"]),
            },
        )
        .expect("valid retry initializes project");
    }
}

#[test]
fn init_rejects_existing_task_project() {
    let temp = project();
    write_config(temp.path(), minimal_config());
    let original = fs::read_to_string(temp.path().join(".tasks/config.json"))
        .expect("configuration should be readable");

    let error = TaskProject::init(temp.path(), InitProjectOptions::default()).unwrap_err();

    assert!(matches!(error, TaskError::ProjectAlreadyExists { .. }));
    assert_eq!(error.code(), "project_already_exists");
    assert_eq!(
        fs::read_to_string(temp.path().join(".tasks/config.json")).unwrap(),
        original
    );
}

#[test]
fn init_command_writes_valid_project() {
    let temp = tempfile::tempdir().expect("temp project created");

    let output = Command::new(env!("CARGO_BIN_EXE_filer-task"))
        .args(["init", "--root"])
        .arg(temp.path())
        .args(["--domain", "work", "--prefix", "WORK,BUG"])
        .output()
        .expect("command runs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let project = TaskProject::open(temp.path()).expect("project opens");
    assert!(
        project
            .policy()
            .domain("work")
            .unwrap()
            .allows_prefix("WORK")
    );
    assert!(
        project
            .policy()
            .domain("work")
            .unwrap()
            .allows_prefix("BUG")
    );
    require_valid_report(validate_repo(&project).expect("project validates"))
        .expect("empty project should be valid");
}

#[test]
fn policy_mutation_adds_domain_prefix_type_and_tag() {
    let temp = project();
    write_config(temp.path(), minimal_config());
    let project = TaskProject::open(temp.path()).expect("project opens");

    let project = project
        .add_domain("docs", &["DOCS".to_string()])
        .expect("domain is added");
    let project = project.add_prefix("work", "BUG").expect("prefix is added");
    let project = project
        .add_task_type("Bug", CriteriaPolicy::Acceptance, None)
        .expect("type is added");
    let project = project.add_tag("backend").expect("tag is added");

    assert!(
        project
            .policy()
            .domain("docs")
            .unwrap()
            .allows_prefix("DOCS")
    );
    assert!(
        project
            .policy()
            .domain("work")
            .unwrap()
            .allows_prefix("BUG")
    );
    assert!(project.policy().task_type("Bug").is_some());
    assert_eq!(
        project.policy().tags(),
        &TagPolicy::Strict {
            allowed: vec!["backend".to_string()]
        }
    );
    require_valid_report(validate_repo(&project).expect("project validates"))
        .expect("empty project should be valid");
}

#[test]
fn policy_mutation_returns_the_only_fresh_policy_handle() {
    let temp = project();
    write_config(temp.path(), minimal_config());
    let project = TaskProject::open(temp.path()).expect("project opens");
    let clone = project.clone();

    let updated = clone
        .add_prefix("work", "BUG")
        .expect("policy mutation succeeds");

    assert!(!updated.is_stale().expect("returned handle is fresh"));
    assert!(project.is_stale().expect("original handle is stale"));
    assert!(clone.is_stale().expect("clone is stale"));
    assert!(matches!(
        project.add_tag("backend"),
        Err(TaskError::StaleProject { .. })
    ));
    updated
        .add_tag("backend")
        .expect("returned handle can mutate policy");
}

#[test]
fn policy_mutation_rejects_removals_used_by_existing_tasks() {
    let temp = project();
    write_config(
        temp.path(),
        r#"{
        "version": 1,
        "domains": {"work": {"prefixes": ["ALT", "WORK"]}},
        "task_types": {
            "Bug": {"criteria": "acceptance"},
            "Feature": {"criteria": "acceptance"}
        },
        "tags": {"policy": "strict", "allowed": ["backend", "unused"]}
    }"#,
    );
    fs::create_dir_all(temp.path().join(".tasks/work")).expect("domain directory created");
    fs::write(
        temp.path().join(".tasks/work/WORK-001-existing-task.md"),
        "---\nid: WORK-001\ntitle: Existing task\nstatus: To Do\npriority: High\ntype: Feature\ntags: [backend]\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("task written");
    let project = TaskProject::open(temp.path()).expect("project opens");
    let original =
        fs::read_to_string(temp.path().join(".tasks/config.json")).expect("configuration readable");

    for error in [
        project.remove_prefix("work", "WORK").unwrap_err(),
        project.remove_task_type("Feature").unwrap_err(),
        project.remove_tag("backend").unwrap_err(),
    ] {
        assert!(matches!(error, TaskError::Validation(_)), "{error:?}");
        assert_eq!(
            fs::read_to_string(temp.path().join(".tasks/config.json")).unwrap(),
            original
        );
    }
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
