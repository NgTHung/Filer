//! # Project Repository
//!
//! This module discovers task projects and reads their task files. Discovery
//! checks only for a `.tasks` directory so validation can report malformed
//! project contents separately.
//!
//! ```
//! use filer_task::repo::discover_project_root;
//!
//! let root = discover_project_root(std::env::current_dir()?)?;
//! assert!(root.join(".tasks").is_dir());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{TaskError, ValidationError},
    project::{CONFIG_PATH, TaskProject},
};

pub const TASK_DIR: &str = ".tasks";
/// Reserved compatibility file that discovery and task loading ignore.
pub const TASK_SCHEMA: &str = "task.schema.json";
pub const MILESTONE_DOMAIN: &str = "milestones";

pub fn discover_project_root(start: impl AsRef<Path>) -> Result<PathBuf, TaskError> {
    let start = start.as_ref();
    let current = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };

    for ancestor in current.ancestors() {
        if ancestor.join(TASK_DIR).is_dir() {
            return Ok(ancestor.to_path_buf());
        }
    }

    Err(TaskError::ProjectNotFound {
        start: start.to_path_buf(),
    })
}

pub fn read_task_files(project: &TaskProject) -> Result<Vec<PathBuf>, TaskError> {
    let mut files = Vec::new();
    for domain in project.policy().domains().keys() {
        let dir = project.root().join(TASK_DIR).join(domain);
        let exists = dir.try_exists().map_err(|source| TaskError::Io {
            path: dir.clone(),
            source,
        })?;
        if !exists {
            continue;
        }
        if !dir.is_dir() {
            continue;
        }
        read_markdown_files(&dir, &mut files)?;
    }
    files.sort();
    Ok(files)
}

pub fn validate_task_layout(project: &TaskProject) -> Result<Vec<ValidationError>, TaskError> {
    let task_root = project.root().join(TASK_DIR);
    let entries = fs::read_dir(&task_root).map_err(|source| TaskError::Io {
        path: task_root.clone(),
        source,
    })?;
    let mut errors = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| TaskError::Io {
            path: task_root.clone(),
            source,
        })?;
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            errors.push(ValidationError::at(
                &path,
                "task project entries must use portable UTF-8 names",
            ));
            continue;
        };
        if !path.is_dir() && is_reserved_entry(&name) {
            continue;
        }
        if path.is_dir() {
            if project.policy().domain(&name).is_none() {
                errors.push(ValidationError::at(
                    &path,
                    format!(
                        "undeclared task domain {name}; configured domains: {}",
                        project
                            .policy()
                            .domains()
                            .keys()
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        } else if project.policy().domain(&name).is_some() {
            errors.push(ValidationError::at(
                &path,
                format!("configured task domain {name} must be a directory"),
            ));
        } else if path.extension().is_some_and(|extension| extension == "md") {
            errors.push(ValidationError::at(
                &path,
                "task Markdown files must live inside a configured domain directory",
            ));
        }
    }
    Ok(errors)
}

fn is_reserved_entry(name: &str) -> bool {
    name == TASK_SCHEMA || CONFIG_PATH.rsplit('/').next() == Some(name)
}

fn read_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), TaskError> {
    let entries = fs::read_dir(dir).map_err(|source| TaskError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| TaskError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            read_markdown_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }

    Ok(())
}
