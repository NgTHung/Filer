//! # Project Freshness
//!
//! This module fingerprints task inputs and coordinates writes through a
//! project-owned lock file. Content hashes make stale checks independent of
//! filesystem timestamp precision.
//!
//! ```
//! use filer_task::project::TaskProject;
//!
//! # let root = tempfile::tempdir()?;
//! # std::fs::create_dir(root.path().join(".tasks"))?;
//! let project = TaskProject::open(root.path())?;
//! assert!(!project.is_stale()?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{error::TaskError, identity::TaskIdentity, repo::TASK_DIR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProjectRevision {
    files: BTreeMap<PathBuf, [u8; 32]>,
}

impl ProjectRevision {
    pub(super) fn read(root: &Path) -> Result<Self, TaskError> {
        let task_root = root.join(TASK_DIR);
        let mut paths = Vec::new();
        collect_inputs(&task_root, &task_root, &mut paths)?;
        paths.sort();

        let mut files = BTreeMap::new();
        for relative in paths {
            let path = task_root.join(&relative);
            let content = fs::read(&path).map_err(|source| TaskError::Io {
                path: path.clone(),
                source,
            })?;
            files.insert(relative, Sha256::digest(content).into());
        }
        Ok(Self { files })
    }

    pub(super) fn task_paths(&self, root: &Path, identity: &TaskIdentity) -> Vec<PathBuf> {
        let expected = format!("{}-", identity.id);
        self.files
            .keys()
            .filter(|relative| {
                relative
                    .components()
                    .next()
                    .is_some_and(|part| part.as_os_str() == identity.domain.as_str())
                    && relative
                        .extension()
                        .is_some_and(|extension| extension == "md")
                    && relative
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(&expected))
            })
            .map(|relative| root.join(TASK_DIR).join(relative))
            .collect()
    }
}

fn collect_inputs(root: &Path, current: &Path, paths: &mut Vec<PathBuf>) -> Result<(), TaskError> {
    let entries = fs::read_dir(current).map_err(|source| TaskError::Io {
        path: current.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| TaskError::Io {
            path: current.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_inputs(root, &path, paths)?;
            continue;
        }
        let is_config = path == root.join("config.json");
        let is_task = path.extension().is_some_and(|extension| extension == "md");
        if is_config || is_task {
            let relative = path.strip_prefix(root).map_err(|error| {
                TaskError::Message(format!("could not fingerprint {}: {error}", path.display()))
            })?;
            paths.push(relative.to_path_buf());
        }
    }
    Ok(())
}
