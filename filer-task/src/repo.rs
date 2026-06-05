use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::TaskError;

pub const TASK_DIR: &str = ".tasks";
pub const TASK_SCHEMA: &str = "task.schema.json";
pub const DOMAINS: &[&str] = &["core", "app", "ecosystem"];

pub fn find_repo_root(start: impl AsRef<Path>) -> Result<PathBuf, TaskError> {
    let start = start.as_ref();
    let mut current = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join(TASK_DIR).join(TASK_SCHEMA).is_file() {
            return Ok(current);
        }

        if !current.pop() {
            return Err(TaskError::MissingRepoRoot {
                start: start.to_path_buf(),
            });
        }
    }
}

pub fn read_task_files(root: &Path) -> Result<Vec<PathBuf>, TaskError> {
    let mut files = Vec::new();
    for domain in DOMAINS {
        let dir = root.join(TASK_DIR).join(domain);
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
