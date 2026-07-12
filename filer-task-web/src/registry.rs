//! # Project Registry
//!
//! Resolves a project name to a repository root. This is the only place that
//! locates a `.tasks/` repo, so growing from one project to many later means
//! adding entries here, not threading a new root through every handler. Handlers
//! already receive a resolved `&Path` and pass it straight into `filer-task`.

use std::path::{Path, PathBuf};

use filer_task::repo::discover_project_root;

use crate::error::WebError;

const DEFAULT_PROJECT: &str = "default";

#[derive(Debug, Clone)]
struct Project {
    name: String,
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectRegistry {
    projects: Vec<Project>,
}

impl ProjectRegistry {
    /// Build a single-project registry from the nearest project containing
    /// `start`.
    pub fn single(start: PathBuf) -> Result<Self, WebError> {
        let root = discover_project_root(&start)?;
        Ok(Self {
            projects: vec![Project {
                name: DEFAULT_PROJECT.to_string(),
                root,
            }],
        })
    }

    /// Resolve a project to its root. `None` selects the default (first) project,
    /// which keeps single-project URLs free of a project segment.
    pub fn resolve(&self, name: Option<&str>) -> Result<&Path, WebError> {
        match name {
            None => self
                .projects
                .first()
                .map(|project| project.root.as_path())
                .ok_or_else(|| WebError::ProjectNotFound(DEFAULT_PROJECT.to_string())),
            Some(name) => self
                .projects
                .iter()
                .find(|project| project.name == name)
                .map(|project| project.root.as_path())
                .ok_or_else(|| WebError::ProjectNotFound(name.to_string())),
        }
    }

    pub fn names(&self) -> Vec<&str> {
        self.projects
            .iter()
            .map(|project| project.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp dir created");
        fs::create_dir_all(temp.path().join(".tasks/core")).expect("core dir created");
        temp
    }

    #[test]
    fn single_resolves_default_to_repo_root() {
        let temp = repo();
        let registry = ProjectRegistry::single(temp.path().to_path_buf()).expect("registry builds");

        let resolved = registry.resolve(None).expect("default resolves");

        assert_eq!(resolved, temp.path());
        assert_eq!(registry.names(), vec!["default"]);
    }

    #[test]
    fn resolving_unknown_project_fails() {
        let temp = repo();
        let registry = ProjectRegistry::single(temp.path().to_path_buf()).expect("registry builds");

        let error = registry.resolve(Some("ghost"));

        assert!(matches!(error, Err(WebError::ProjectNotFound(name)) if name == "ghost"));
    }

    #[test]
    fn single_fails_without_a_repo() {
        let temp = tempfile::tempdir().expect("temp dir created");

        let error = ProjectRegistry::single(temp.path().to_path_buf());

        assert!(error.is_err());
    }
}
