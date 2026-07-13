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

use crate::{error::TaskError, project::TaskProject};

pub const TASK_DIR: &str = ".tasks";
/// Reserved compatibility file that discovery and task loading ignore.
pub const TASK_SCHEMA: &str = "task.schema.json";
pub const DOMAINS: &[&str] = &["core", "app", "ecosystem"];
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
    for domain in DOMAINS.iter().copied().chain([MILESTONE_DOMAIN]) {
        let dir = project.root().join(TASK_DIR).join(domain);
        if !dir.exists() {
            continue;
        }

        read_markdown_files(&dir, &mut files)?;
    }
    files.sort();
    Ok(files)
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
