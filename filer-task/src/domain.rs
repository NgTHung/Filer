//! # Task Domains
//!
//! This module keeps domain path checks and compatibility-prefix data outside
//! the repository-wide validator. Configured projects use their declared
//! domains, while projects without configuration retain the Filer layout.
//!
//! ```
//! use filer_task::project::TaskProject;
//!
//! let root = tempfile::tempdir()?;
//! std::fs::create_dir(root.path().join(".tasks"))?;
//! let project = TaskProject::open(root.path())?;
//! assert!(!project.policy().domains().is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::HashSet;

use crate::{error::ValidationError, model::Task, project::TaskProject, repo::TASK_DIR};

pub(crate) const CORE_PREFIXES: &[&str] = &[
    "CORE", "ACTORS", "API", "MODULES", "PIPELINE", "SERVICES", "UTILS", "VFS", "REL", "NAV",
    "SEARCH", "OPS", "PREVIEW", "PROVIDER", "PROTOCOL",
];
pub(crate) const APP_PREFIXES: &[&str] =
    &["UI", "EXPL", "SETS", "SRCH", "MEDIA", "NAV", "PERF", "A11Y"];
pub(crate) const ECOSYSTEM_PREFIXES: &[&str] = &["PLUG", "EXT", "THEME", "PROFILE", "PROVIDER"];

pub(crate) fn validate_task_path(
    project: &TaskProject,
    task: &Task,
    errors: &mut Vec<ValidationError>,
) {
    let path = &task.path;
    let relative = match path.strip_prefix(project.root()) {
        Ok(relative) => relative,
        Err(_) => {
            errors.push(ValidationError::at(path, "task file is outside repo root"));
            return;
        }
    };
    let mut parts = relative.components();
    let task_dir = parts.next().and_then(|part| part.as_os_str().to_str());
    let domain = parts.next().and_then(|part| part.as_os_str().to_str());
    if task_dir != Some(TASK_DIR)
        || domain.is_none_or(|value| project.policy().domain(value).is_none())
    {
        errors.push(ValidationError::at(
            path,
            "task file must live under a configured .tasks domain",
        ));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let expected_prefix = format!("{}-", task.metadata.id);
    if !file_name.starts_with(&expected_prefix) {
        errors.push(ValidationError::at(
            path,
            format!("file name must start with {expected_prefix}"),
        ));
    }
}

pub(crate) fn validate_duplicate_identities(tasks: &[Task], errors: &mut Vec<ValidationError>) {
    let mut identities = HashSet::new();
    let mut duplicates = HashSet::new();
    for task in tasks {
        let identity = (task.domain.as_str(), task.metadata.id.as_str());
        if !identities.insert(identity) {
            duplicates.insert(identity);
        }
    }
    for (domain, local_id) in duplicates {
        errors.push(ValidationError::new(
            None,
            format!("duplicate task id {domain}:{local_id}"),
        ));
    }
}
