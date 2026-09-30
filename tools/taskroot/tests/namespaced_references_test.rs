use std::{fs, path::Path, process::Command};

use taskroot::{
    identity::{TaskIdentity, TaskReference},
    model::{Priority, Task, TaskMetadata, TaskStatus, TaskType},
    project::TaskProject,
    reference::IdentityIndex,
    validate::{require_valid_report, validate_repo},
};
use tempfile::TempDir;

#[test]
fn task_references_parse_display_and_serialize_as_frontmatter_strings() {
    let local = TaskReference::parse("WORK-001").expect("local reference parses");
    let qualified = TaskReference::parse("backend:WORK-001").expect("qualified reference parses");

    assert_eq!(local.to_string(), "WORK-001");
    assert_eq!(qualified.to_string(), "backend:WORK-001");
    assert_eq!(
        serde_json::to_string(&local).expect("local reference serializes"),
        r#""WORK-001""#
    );
    assert_eq!(
        serde_json::from_str::<TaskReference>(r#""backend:WORK-001""#)
            .expect("qualified reference deserializes"),
        qualified
    );
    assert!(TaskReference::parse("backend:").is_err());
    assert!(TaskReference::parse("work-item").is_err());
}

#[test]
fn task_identity_is_derived_and_serialized_without_redundant_state() {
    let task = Task {
        path: "task.md".into(),
        domain: "backend".to_string(),
        metadata: TaskMetadata {
            id: "WORK-001".to_string(),
            title: "Backend task".to_string(),
            status: TaskStatus::ToDo,
            priority: Priority::High,
            task_type: TaskType::new("Feature"),
            parent: None,
            milestone: None,
            depends_on: Vec::new(),
            rules: Vec::new(),
            risk: None,
            impact: None,
            tags: Vec::new(),
            whitepaper: None,
            last_updated: None,
        },
    };

    assert_eq!(
        task.identity(),
        TaskIdentity::new("backend", "WORK-001").expect("identity is valid")
    );
    assert_eq!(task.qualified_id(), "backend:WORK-001");
    let json = serde_json::to_value(&task).expect("task serializes");
    assert_eq!(json["id"], "WORK-001");
    assert_eq!(json["domain"], "backend");
    assert_eq!(json["qualified_id"], "backend:WORK-001");
}

#[test]
fn configured_projects_resolve_local_relationships_only_in_the_source_domain() {
    let repo = configured_project(&[("backend", "WORK"), ("release", "WORK")]);
    write_task(repo.path(), "release", "WORK-001", "Release parent", "");
    write_task(
        repo.path(),
        "backend",
        "WORK-002",
        "Backend child",
        "parent: WORK-001\n",
    );

    let project = TaskProject::open(repo.path()).expect("project opens");
    let report = validate_repo(&project).expect("repository scan succeeds");

    assert!(report.warnings.is_empty());
    assert!(report.errors.iter().any(|error| {
        error
            .message
            .contains("parent WORK-001 does not reference a task in domain backend")
    }));
}

#[test]
fn compatibility_fallback_is_retained_as_a_structured_warning() {
    let repo = compatibility_project();
    write_task(
        repo.path(),
        "milestones",
        "MILESTONE-003",
        "Core milestone",
        "milestone: \"0.3.0\"\n",
    );
    rewrite_type(repo.path(), "milestones", "MILESTONE-003", "Milestone");
    write_task(
        repo.path(),
        "core",
        "CORE-001",
        "Core child",
        "parent: MILESTONE-003\n",
    );

    let project = TaskProject::open(repo.path()).expect("project opens");
    let validated =
        require_valid_report(validate_repo(&project).expect("repository scan succeeds"))
            .expect("legacy reference remains valid");

    assert_eq!(validated.tasks.len(), 2);
    assert_eq!(validated.warnings.len(), 1);
    let warning = &validated.warnings[0];
    assert_eq!(warning.code, "legacy_global_reference");
    assert_eq!(warning.context["reference"], "MILESTONE-003");
    assert_eq!(warning.context["source_domain"], "core");
    assert_eq!(
        warning.context["resolved_identity"],
        "milestones:MILESTONE-003"
    );
}

#[test]
fn qualified_selectors_and_graph_relations_target_exact_duplicate_ids() {
    let repo = configured_project(&[("backend", "WORK"), ("release", "WORK")]);
    write_task(repo.path(), "backend", "WORK-001", "Backend parent", "");
    write_task(repo.path(), "release", "WORK-001", "Release dependency", "");
    write_task(
        repo.path(),
        "backend",
        "WORK-002",
        "Backend target",
        "parent: WORK-001\ndepends_on: [release:WORK-001]\n",
    );
    write_task(
        repo.path(),
        "release",
        "WORK-002",
        "Release child",
        "parent: WORK-001\n",
    );
    write_task(
        repo.path(),
        "release",
        "WORK-003",
        "Cross domain dependent",
        "depends_on: [backend:WORK-002]\n",
    );

    for command in ["show", "context", "deps"] {
        let output = run(repo.path(), &[command, "WORK-001"]);
        assert!(
            !output.status.success(),
            "{command} rejects local selectors"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("domain is required"));
        assert!(stderr.contains("backend:WORK-001"));
        assert!(stderr.contains("release:WORK-001"));
    }

    let context = run_json(
        repo.path(),
        &["context", "backend:WORK-002", "--format", "json"],
    );
    assert_eq!(
        context["parent"]["task"]["qualified_id"],
        "backend:WORK-001"
    );
    assert_eq!(
        context["dependencies"][0]["task"]["qualified_id"],
        "release:WORK-001"
    );
    assert_eq!(
        context["dependents"][0]["task"]["qualified_id"],
        "release:WORK-003"
    );
    assert_eq!(
        context["readiness"]["blockers"][0]["task_id"],
        "release:WORK-001"
    );

    let deps = run_json(
        repo.path(),
        &["deps", "backend:WORK-002", "--format", "json"],
    );
    assert_eq!(deps[0]["qualified_id"], "release:WORK-001");

    let children = run_json(
        repo.path(),
        &["list", "--parent", "backend:WORK-001", "--format", "json"],
    );
    assert_eq!(children.as_array().expect("task array").len(), 1);
    assert_eq!(children[0]["qualified_id"], "backend:WORK-002");

    let parent_context = run_json(
        repo.path(),
        &["context", "backend:WORK-001", "--format", "json"],
    );
    assert_eq!(
        parent_context["children"]
            .as_array()
            .expect("children")
            .len(),
        1
    );
    assert_eq!(
        parent_context["children"][0]["task"]["qualified_id"],
        "backend:WORK-002"
    );
}

#[test]
fn every_lifecycle_command_mutates_only_the_qualified_domain() {
    let repo = configured_project(&[("backend", "WORK"), ("release", "WORK")]);
    for number in 1..=5 {
        let id = format!("WORK-{number:03}");
        write_task(repo.path(), "backend", &id, "Backend lifecycle", "");
        write_task(repo.path(), "release", &id, "Release lifecycle", "");
    }
    check_criteria(repo.path(), "backend", "WORK-002");

    let commands: [(&[&str], &str, &str); 5] = [
        (&["start", "backend:WORK-001"], "WORK-001", "In Progress"),
        (&["done", "backend:WORK-002"], "WORK-002", "Done"),
        (
            &["block", "backend:WORK-003", "Waiting for policy."],
            "WORK-003",
            "Blocked",
        ),
        (
            &["defer", "backend:WORK-004", "Outside this release."],
            "WORK-004",
            "Deferred",
        ),
        (
            &["obsolete", "backend:WORK-005", "Replaced by new work."],
            "WORK-005",
            "Obsolete",
        ),
    ];
    for (args, id, expected_status) in commands {
        let output = run(repo.path(), args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("backend:{id}")));
        assert_eq!(read_status(repo.path(), "backend", id), expected_status);
        assert_eq!(read_status(repo.path(), "release", id), "To Do");
    }

    let rejected = run(repo.path(), &["start", "WORK-001"]);
    assert!(!rejected.status.success());
    assert_eq!(read_status(repo.path(), "release", "WORK-001"), "To Do");
}

#[test]
fn creation_resolves_local_and_qualified_relationships_without_fallback() {
    let repo = configured_project(&[("backend", "WORK"), ("release", "WORK")]);
    write_task(repo.path(), "backend", "WORK-001", "Backend parent", "");
    write_task(repo.path(), "release", "WORK-001", "Release dependency", "");

    let created = run(
        repo.path(),
        &[
            "add",
            "--id",
            "backend:WORK-002",
            "--title",
            "Namespaced creation",
            "--priority",
            "High",
            "--type",
            "Feature",
            "--parent",
            "WORK-001",
            "--depends-on",
            "release:WORK-001",
        ],
    );
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let content = fs::read_to_string(
        repo.path()
            .join(".tasks/backend/WORK-002-namespaced-creation.md"),
    )
    .expect("created task read");
    assert!(content.contains("parent: \"WORK-001\""));
    assert!(content.contains("depends_on: [\"release:WORK-001\"]"));

    let missing = run(
        repo.path(),
        &[
            "add",
            "--id",
            "backend:WORK-003",
            "--title",
            "Strict local creation",
            "--priority",
            "High",
            "--type",
            "Feature",
            "--parent",
            "WORK-999",
        ],
    );
    assert!(!missing.status.success());
    assert!(
        !repo
            .path()
            .join(".tasks/backend/WORK-003-strict-local-creation.md")
            .exists()
    );

    let duplicate = run(
        repo.path(),
        &[
            "add",
            "--id",
            "backend:WORK-004",
            "--title",
            "Duplicate dependencies",
            "--priority",
            "High",
            "--type",
            "Feature",
            "--depends-on",
            "WORK-001,backend:WORK-001",
        ],
    );
    assert!(!duplicate.status.success());
    assert!(
        String::from_utf8_lossy(&duplicate.stderr)
            .contains("duplicate dependency backend:WORK-001")
    );

    let self_dependency = run(
        repo.path(),
        &[
            "add",
            "--id",
            "backend:WORK-005",
            "--title",
            "Self dependency",
            "--priority",
            "High",
            "--type",
            "Feature",
            "--depends-on",
            "backend:WORK-005",
        ],
    );
    assert!(!self_dependency.status.success());
    assert!(String::from_utf8_lossy(&self_dependency.stderr).contains("depend on itself"));
}

#[test]
fn import_preserves_local_relationships_and_canonicalizes_qualified_ones() {
    let repo = configured_project(&[("backend", "WORK"), ("release", "WORK")]);
    write_task(repo.path(), "backend", "WORK-001", "Backend parent", "");
    write_task(repo.path(), "release", "WORK-001", "Release dependency", "");
    let import = repo.path().join("relationships.json");
    fs::write(
        &import,
        r#"[{
          "domain":"backend",
          "id":"WORK-002",
          "title":"Imported relationships",
          "priority":"High",
          "type":"Feature",
          "parent":"WORK-001",
          "depends_on":["release:WORK-001"]
        }]"#,
    )
    .expect("import written");

    let output = run(
        repo.path(),
        &["import", import.to_str().expect("UTF-8 path")],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = fs::read_to_string(
        repo.path()
            .join(".tasks/backend/WORK-002-imported-relationships.md"),
    )
    .expect("imported task read");
    assert!(content.contains("parent: \"WORK-001\""));
    assert!(content.contains("depends_on: [\"release:WORK-001\"]"));
}

#[test]
fn import_detects_exact_cross_domain_cycles_before_writing() {
    let repo = configured_project(&[("backend", "WORK"), ("release", "WORK")]);
    let import = repo.path().join("cycle.json");
    fs::write(
        &import,
        r#"[
          {"domain":"backend","id":"WORK-001","title":"Backend batch task","priority":"High","type":"Feature","depends_on":["release:WORK-001"]},
          {"domain":"release","id":"WORK-001","title":"Release batch task","priority":"High","type":"Feature","depends_on":["backend:WORK-001"]}
        ]"#,
    )
    .expect("import written");

    let output = run(
        repo.path(),
        &["import", import.to_str().expect("UTF-8 path")],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("backend:WORK-001 -> release:WORK-001 -> backend:WORK-001")
    );
    assert!(
        !repo
            .path()
            .join(".tasks/backend/WORK-001-backend-batch-task.md")
            .exists()
    );
    assert!(
        !repo
            .path()
            .join(".tasks/release/WORK-001-release-batch-task.md")
            .exists()
    );
}

#[test]
fn compatibility_fallback_rejects_ambiguous_project_wide_matches() {
    let repo = compatibility_project();
    write_task(repo.path(), "core", "NAV-001", "Core navigation", "");
    write_task(repo.path(), "app", "NAV-001", "App navigation", "");
    write_task(
        repo.path(),
        "ecosystem",
        "PLUG-001",
        "Plugin task",
        "depends_on: [NAV-001]\n",
    );

    let project = TaskProject::open(repo.path()).expect("project opens");
    let report = validate_repo(&project).expect("repository scan succeeds");
    assert!(report.errors.iter().any(|error| {
        error.message.contains("dependency NAV-001 is ambiguous")
            && error.message.contains("app:NAV-001")
            && error.message.contains("core:NAV-001")
    }));
}

#[test]
fn version_two_outputs_expose_canonical_ids_and_validation_warnings() {
    let repo = compatibility_project();
    write_task(
        repo.path(),
        "milestones",
        "MILESTONE-003",
        "Core milestone",
        "milestone: \"0.3.0\"\n",
    );
    rewrite_type(repo.path(), "milestones", "MILESTONE-003", "Milestone");
    write_task(
        repo.path(),
        "core",
        "CORE-001",
        "Core child",
        "parent: MILESTONE-003\n",
    );

    let validation = run_json(repo.path(), &["validate", "--format", "json"]);
    assert_eq!(validation["task_count"], 2);
    assert_eq!(validation["warnings"][0]["code"], "legacy_global_reference");
    assert_eq!(
        validation["warnings"][0]["context"]["resolved_identity"],
        "milestones:MILESTONE-003"
    );

    let human_validation = run(repo.path(), &["validate"]);
    let human_validation = String::from_utf8(human_validation.stdout).expect("UTF-8 output");
    assert!(human_validation.contains("Warnings: 1"));
    assert!(human_validation.contains("legacy_global_reference"));

    for command in ["show", "context"] {
        let output = run_json(repo.path(), &[command, "core:CORE-001", "--format", "json"]);
        assert_eq!(output["schema_version"], 2);
        assert_eq!(output["warnings"][0]["code"], "legacy_global_reference");
        assert_eq!(output["detail"]["task"]["qualified_id"], "core:CORE-001");
        assert_eq!(
            output["detail"]["task"]["parent"],
            "milestones:MILESTONE-003"
        );
    }

    let ready = run_json(repo.path(), &["ready", "--format", "json"]);
    assert_eq!(ready["schema_version"], 2);
    assert_eq!(ready["warnings"][0]["code"], "legacy_global_reference");

    let list = run(repo.path(), &["list"]);
    let list = String::from_utf8(list.stdout).expect("UTF-8 output");
    assert!(list.contains("core:CORE-001"));
    assert!(list.contains("milestones:MILESTONE-003"));
}

#[test]
fn namespace_errors_and_help_expose_actionable_identity_context() {
    let index = IdentityIndex::new(
        "/project",
        [
            TaskIdentity::new("backend", "WORK-001").expect("valid identity"),
            TaskIdentity::new("release", "WORK-001").expect("valid identity"),
        ],
    );
    let required = index
        .resolve_cli("WORK-001")
        .expect_err("domain is required");
    assert_eq!(required.code(), "domain_required");
    assert_eq!(required.context()["candidates"][0], "backend:WORK-001");
    assert_eq!(required.context()["candidates"][1], "release:WORK-001");

    let missing = index
        .resolve_cli("backend:WORK-999")
        .expect_err("qualified task is missing");
    assert_eq!(missing.code(), "task_not_found");
    assert_eq!(missing.context()["reference"], "backend:WORK-999");

    let malformed = index
        .resolve_cli("backend:")
        .expect_err("malformed selector is rejected");
    assert_eq!(malformed.code(), "invalid_reference");
    assert_eq!(malformed.context()["reference"], "backend:");

    for command in ["show", "context", "deps", "start", "done"] {
        let output = Command::new(env!("CARGO_BIN_EXE_taskroot"))
            .args([command, "--help"])
            .output()
            .expect("help command runs");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("domain:LOCAL-ID"));
    }
}

#[test]
fn project_wide_resolution_accepts_qualified_and_unique_bare_selectors() {
    let index = IdentityIndex::new(
        "/project",
        [
            TaskIdentity::new("backend", "WORK-001").expect("valid identity"),
            TaskIdentity::new("release", "WORK-002").expect("valid identity"),
        ],
    );

    assert_eq!(
        index
            .resolve_project_wide("backend:WORK-001")
            .expect("qualified identity resolves")
            .to_string(),
        "backend:WORK-001"
    );
    assert_eq!(
        index
            .resolve_project_wide("WORK-002")
            .expect("unique bare identity resolves")
            .to_string(),
        "release:WORK-002"
    );
}

#[test]
fn project_wide_resolution_reports_ambiguous_and_missing_selectors() {
    let index = IdentityIndex::new(
        "/project",
        [
            TaskIdentity::new("release", "WORK-001").expect("valid identity"),
            TaskIdentity::new("backend", "WORK-001").expect("valid identity"),
        ],
    );

    let ambiguous = index
        .resolve_project_wide("WORK-001")
        .expect_err("duplicate bare identity is ambiguous");
    assert_eq!(ambiguous.code(), "ambiguous_reference");
    assert_eq!(
        ambiguous.context()["candidates"],
        serde_json::json!(["backend:WORK-001", "release:WORK-001"])
    );

    let missing = index
        .resolve_project_wide("WORK-999")
        .expect_err("missing bare identity is rejected");
    assert_eq!(missing.code(), "task_not_found");
    assert_eq!(missing.context()["reference"], "WORK-999");
}

fn configured_project(domains: &[(&str, &str)]) -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
    let domains = domains
        .iter()
        .map(|(domain, prefix)| format!(r#""{domain}":{{"prefixes":["{prefix}"]}}"#))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        repo.path().join(".tasks/config.json"),
        format!(
            r#"{{"version":1,"domains":{{{domains}}},"task_types":{{"Feature":{{"criteria":"acceptance"}}}},"tags":{{"policy":"open"}}}}"#
        ),
    )
    .expect("configuration written");
    repo
}

fn compatibility_project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
    repo
}

fn write_task(root: &Path, domain: &str, id: &str, title: &str, extra: &str) {
    let directory = root.join(".tasks").join(domain);
    fs::create_dir_all(&directory).expect("domain directory created");
    fs::write(
        directory.join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n{extra}---\n\n## Summary\n\nExercise namespaced references.\n\n## Acceptance Criteria\n\n- [ ] Works.\n"
        ),
    )
    .expect("task written");
}

fn rewrite_type(root: &Path, domain: &str, id: &str, task_type: &str) {
    let path = root
        .join(".tasks")
        .join(domain)
        .join(format!("{id}-task.md"));
    let content = fs::read_to_string(&path).expect("task read");
    let content = content
        .replace("type: Feature", &format!("type: {task_type}"))
        .replace("## Acceptance Criteria", "## Exit Criteria");
    fs::write(path, content).expect("task rewritten");
}

fn check_criteria(root: &Path, domain: &str, id: &str) {
    let path = root
        .join(".tasks")
        .join(domain)
        .join(format!("{id}-task.md"));
    let content = fs::read_to_string(&path).expect("task read");
    fs::write(path, content.replace("- [ ] Works.", "- [x] Works.")).expect("criteria checked");
}

fn read_status(root: &Path, domain: &str, id: &str) -> String {
    let path = root
        .join(".tasks")
        .join(domain)
        .join(format!("{id}-task.md"));
    fs::read_to_string(path)
        .expect("task read")
        .lines()
        .find_map(|line| line.strip_prefix("status: "))
        .expect("status exists")
        .to_string()
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_taskroot"))
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .expect("taskroot command runs")
}

fn run_json(root: &Path, args: &[&str]) -> serde_json::Value {
    let output = run(root, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("command returns JSON")
}
