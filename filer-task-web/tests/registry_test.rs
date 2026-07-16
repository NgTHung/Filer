use std::{fs, sync::Arc};

use filer_task::project::TaskProject;
use filer_task_web::registry::ProjectRegistry;
use tempfile::TempDir;

#[test]
fn registry_registers_projects_and_replaces_open_handles() {
    let initial = project();
    let added = project();
    let registry = ProjectRegistry::single(initial.path().to_path_buf()).expect("registry builds");
    let added_project = TaskProject::open(added.path()).expect("added project opens");

    let registered = registry.register(added_project).expect("project registers");

    assert_eq!(registered.name(), project_name(&added));
    assert_eq!(
        registry.names(),
        vec![project_name(&initial), project_name(&added)]
    );
    let fresh = registered
        .task_project()
        .add_domain("backend", &["API".to_string()])
        .expect("policy changes");
    let write_lock = registered.write_lock();
    registry
        .replace_task_project(registered.name(), fresh)
        .expect("project handle replaced");
    let resolved = registry
        .resolve(registered.name())
        .expect("project resolves");
    assert!(resolved.task_project().policy().domain("backend").is_some());
    assert!(Arc::ptr_eq(&write_lock, &resolved.write_lock()));
}

fn project() -> TempDir {
    let project = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(project.path().join(".tasks")).expect("task directory created");
    project
}

fn project_name(project: &TempDir) -> String {
    project
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name")
        .to_string()
}
