use std::{fs, path::Path, process::Command};

use serde_json::Value;
use taskroot::{project::TaskProject, validate::validate_repo};
use tempfile::TempDir;

fn project(domains: &[(&str, &str)]) -> TempDir {
    let temp = tempfile::tempdir().expect("temp project created");
    fs::create_dir(temp.path().join(".tasks")).expect("task directory created");
    let domains = domains
        .iter()
        .map(|(domain, prefix)| format!(r#""{domain}": {{"prefixes": ["{prefix}"]}}"#))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        temp.path().join(".tasks/config.json"),
        format!(r#"{{"version":1,"domains":{{{domains}}},"task_types":{{"Feature":{{"criteria":"acceptance"}}}},"tags":{{"policy":"open"}}}}"#),
    )
    .expect("configuration written");
    temp
}

fn write_task(root: &Path, domain: &str, id: &str, suffix: &str) {
    let directory = root.join(".tasks").join(domain);
    fs::create_dir_all(&directory).expect("domain directory created");
    fs::write(
        directory.join(format!("{id}-{suffix}.md")),
        format!(
            "---\nid: {id}\ntitle: Domain task {suffix}\nstatus: To Do\npriority: High\n\
             type: Feature\n---\n\n## Summary\n\nExercise project domain behavior.\n\n\
             ## Acceptance Criteria\n\n- [ ] Domain behavior works.\n"
        ),
    )
    .expect("task written");
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_taskroot"))
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .expect("taskroot command should run")
}

fn add(root: &Path, domain: Option<&str>, id: &str) -> std::process::Output {
    let mut args = vec![
        "add",
        "--id",
        id,
        "--title",
        "Domain task title",
        "--priority",
        "High",
        "--type",
        "Feature",
    ];
    if let Some(domain) = domain {
        args.extend(["--domain", domain]);
    }
    run(root, &args)
}

#[test]
fn configured_domains_load_exact_local_ids_and_cross_domain_duplicates() {
    let repo = project(&[("backend", "WORK"), ("release", "WORK")]);
    write_task(repo.path(), "backend", "WORK-001", "backend");
    write_task(repo.path(), "backend", "WORK-1", "short-number");
    let created = add(repo.path(), None, "release:WORK-001");
    assert!(created.status.success());

    let output = run(
        repo.path(),
        &["list", "--sort-by", "id", "--format", "json"],
    );

    assert!(output.status.success());
    let tasks: Value = serde_json::from_slice(&output.stdout).expect("list output should be JSON");
    let tasks = tasks.as_array().expect("list output should be an array");
    assert_eq!(tasks.len(), 3);
    for (domain, id) in [
        ("backend", "WORK-001"),
        ("backend", "WORK-1"),
        ("release", "WORK-001"),
    ] {
        assert!(
            tasks
                .iter()
                .any(|task| task["domain"] == domain && task["id"] == id)
        );
    }

    let filtered = run(
        repo.path(),
        &["list", "--domain", "release", "--format", "json"],
    );
    let tasks: Value = serde_json::from_slice(&filtered.stdout).expect("filtered output is JSON");
    assert_eq!(tasks.as_array().expect("array output").len(), 1);
    assert_eq!(tasks[0]["domain"], "release");
    assert_eq!(tasks[0]["id"], "WORK-001");
}

#[test]
fn add_accepts_qualified_and_explicit_default_domain_forms() {
    let repo = project(&[("default", "WORK")]);

    let qualified = add(repo.path(), Some("default"), "default:WORK-001");
    assert!(qualified.status.success());

    let explicit = add(repo.path(), Some("default"), "WORK-002");
    assert!(explicit.status.success());
}

#[test]
fn add_requires_a_domain_and_rejects_conflicts() {
    let repo = project(&[("default", "WORK"), ("release", "WORK")]);
    let required = add(repo.path(), None, "WORK-001");
    assert!(!required.status.success());
    assert!(String::from_utf8_lossy(&required.stderr).contains("domain is required"));

    let conflict = add(repo.path(), Some("release"), "default:WORK-001");
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("conflicts with"));
}

#[test]
fn add_rejects_invalid_and_windows_device_domains() {
    let repo = project(&[("work", "WORK")]);
    for domain in ["Core", "con", "nul", "com1", "lpt1"] {
        let id = format!("{domain}:WORK-001");
        let output = add(repo.path(), None, &id);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(domain));
    }
}

#[test]
fn layout_ignores_reserved_files_and_reports_undeclared_domains() {
    let repo = project(&[("work", "WORK")]);
    fs::write(repo.path().join(".tasks/task.schema.json"), "{}")
        .expect("compatibility schema written");
    write_task(repo.path(), "work", "WORK-001", "configured");
    write_task(repo.path(), "ghost", "WORK-002", "undeclared");

    let project = TaskProject::open(repo.path()).expect("project opens");
    let report = validate_repo(&project).expect("repository scan succeeds");

    assert!(
        report
            .errors
            .iter()
            .any(|error| error.message.contains("undeclared task domain ghost"))
    );
    assert!(
        !report
            .errors
            .iter()
            .any(|error| error.message.contains("task.schema.json"))
    );
    assert_eq!(report.tasks.len(), 1);
    assert_eq!(report.tasks[0].domain, "work");
}
