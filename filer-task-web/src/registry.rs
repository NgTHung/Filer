//! # Project Registry
//!
//! Discovers configured task roots and gives each project an independent write
//! boundary. Repository contents stay uncached so CLI and git edits are visible
//! on the next request.
//!
//! ```no_run
//! use filer_task_web::registry::ProjectRegistry;
//!
//! let registry = ProjectRegistry::single(std::env::current_dir().unwrap());
//! assert!(registry.is_ok());
//! ```

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use filer_task::{
    project::TaskProject,
    repo::discover_project_root,
    validate::{ValidatedRepository, validate_repo},
};
use tokio::sync::Mutex;

use crate::{
    dto::{ProjectSummary, ValidationIssue},
    error::WebError,
};

#[derive(Debug, Clone)]
pub struct RegisteredProject {
    name: String,
    task_project: TaskProject,
    write_lock: Arc<Mutex<()>>,
}

impl RegisteredProject {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn task_project(&self) -> &TaskProject {
        &self.task_project
    }

    pub fn write_lock(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.write_lock)
    }

    pub fn validate(&self) -> Result<ValidatedRepository, WebError> {
        let report = validate_repo(&self.task_project)?;
        if report.errors.is_empty() {
            Ok(ValidatedRepository {
                tasks: report.tasks,
                warnings: report.warnings,
            })
        } else {
            Err(WebError::ProjectBroken {
                name: self.name.clone(),
                issues: report
                    .errors
                    .into_iter()
                    .map(ValidationIssue::from)
                    .collect(),
            })
        }
    }

    fn summary(&self) -> Result<ProjectSummary, WebError> {
        let report = validate_repo(&self.task_project)?;
        let issues: Vec<_> = report
            .errors
            .into_iter()
            .map(ValidationIssue::from)
            .collect();
        Ok(ProjectSummary {
            name: self.name.clone(),
            task_count: report.tasks.len(),
            domain_count: self.task_project.policy().domains().len(),
            broken: !issues.is_empty(),
            issues,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProjectRegistry {
    projects: Vec<RegisteredProject>,
}

impl ProjectRegistry {
    /// Build a single-project registry from the nearest project containing
    /// `start`.
    pub fn single(start: PathBuf) -> Result<Self, WebError> {
        Self::from_roots([start])
    }

    /// Discover and open every configured project start path.
    pub fn from_roots(starts: impl IntoIterator<Item = PathBuf>) -> Result<Self, WebError> {
        let mut projects = Vec::new();
        let mut names = HashSet::new();
        for start in starts {
            let root = discover_project_root(&start)?;
            let task_project = TaskProject::open(root)?;
            let name = task_project
                .root()
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| WebError::InvalidProjectName(task_project.root().to_path_buf()))?
                .to_string();
            if !names.insert(name.clone()) {
                return Err(WebError::DuplicateProjectName(name));
            }
            projects.push(RegisteredProject {
                name,
                task_project,
                write_lock: Arc::new(Mutex::new(())),
            });
        }
        if projects.is_empty() {
            return Err(WebError::NoProjects);
        }
        Ok(Self { projects })
    }

    pub fn resolve(&self, name: &str) -> Result<&RegisteredProject, WebError> {
        self.projects
            .iter()
            .find(|project| project.name == name)
            .ok_or_else(|| WebError::ProjectNotFound(name.to_string()))
    }

    pub fn summaries(&self) -> Result<Vec<ProjectSummary>, WebError> {
        self.projects
            .iter()
            .map(RegisteredProject::summary)
            .collect()
    }

    pub fn names(&self) -> Vec<&str> {
        self.projects
            .iter()
            .map(|project| project.name.as_str())
            .collect()
    }
}
