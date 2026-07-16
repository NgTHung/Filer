//! # Project Registry
//!
//! Discovers configured task roots and gives each project an independent write
//! boundary. Repository contents stay uncached so CLI and git edits are visible
//! on the next request.
//!
//! ```no_run
//! use filer_task_web::registry::ProjectRegistry;
//!
//! if let Ok(current) = std::env::current_dir() {
//!     assert!(ProjectRegistry::single(current).is_ok());
//! }
//! ```

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

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
    fn new(task_project: TaskProject) -> Result<Self, WebError> {
        let name = project_name(&task_project)?;
        Ok(Self {
            name,
            task_project,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

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
        self.validate_project(&self.task_project)
    }

    pub(crate) fn validate_project(
        &self,
        project: &TaskProject,
    ) -> Result<ValidatedRepository, WebError> {
        let report = validate_repo(project)?;
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

    pub(crate) fn summary(&self) -> Result<ProjectSummary, WebError> {
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
    projects: Arc<RwLock<Vec<RegisteredProject>>>,
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
            let registered = RegisteredProject::new(task_project)?;
            let name = registered.name.clone();
            if !names.insert(name.clone()) {
                return Err(WebError::DuplicateProjectName(name));
            }
            projects.push(registered);
        }
        if projects.is_empty() {
            return Err(WebError::NoProjects);
        }
        Ok(Self {
            projects: Arc::new(RwLock::new(projects)),
        })
    }

    pub fn resolve(&self, name: &str) -> Result<RegisteredProject, WebError> {
        read_projects(&self.projects)
            .iter()
            .find(|project| project.name == name)
            .cloned()
            .ok_or_else(|| WebError::ProjectNotFound(name.to_string()))
    }

    pub fn summaries(&self) -> Result<Vec<ProjectSummary>, WebError> {
        let projects = read_projects(&self.projects).clone();
        projects.iter().map(RegisteredProject::summary).collect()
    }

    pub fn names(&self) -> Vec<String> {
        read_projects(&self.projects)
            .iter()
            .map(|project| project.name.clone())
            .collect()
    }

    /// Register an opened project while preserving insertion order.
    pub fn register(&self, task_project: TaskProject) -> Result<RegisteredProject, WebError> {
        let registered = RegisteredProject::new(task_project)?;
        let mut projects = write_projects(&self.projects);
        if projects
            .iter()
            .any(|project| project.name == registered.name)
        {
            return Err(WebError::DuplicateProjectName(registered.name));
        }
        projects.push(registered.clone());
        Ok(registered)
    }

    /// Publish a fresh handle without changing the project's write boundary.
    pub fn replace_task_project(
        &self,
        name: &str,
        task_project: TaskProject,
    ) -> Result<RegisteredProject, WebError> {
        let replacement_name = project_name(&task_project)?;
        if replacement_name != name {
            return Err(WebError::BadRequest(format!(
                "replacement project {replacement_name:?} does not match registered name {name:?}"
            )));
        }
        let mut projects = write_projects(&self.projects);
        let registered = projects
            .iter_mut()
            .find(|project| project.name == name)
            .ok_or_else(|| WebError::ProjectNotFound(name.to_string()))?;
        if registered.task_project.root() != task_project.root() {
            return Err(WebError::BadRequest(format!(
                "replacement root {} does not match registered root {}",
                task_project.root().display(),
                registered.task_project.root().display()
            )));
        }
        registered.task_project = task_project;
        Ok(registered.clone())
    }
}

fn project_name(project: &TaskProject) -> Result<String, WebError> {
    project
        .root()
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| WebError::InvalidProjectName(project.root().to_path_buf()))
}

fn read_projects(
    lock: &RwLock<Vec<RegisteredProject>>,
) -> RwLockReadGuard<'_, Vec<RegisteredProject>> {
    match lock.read() {
        Ok(projects) => projects,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_projects(
    lock: &RwLock<Vec<RegisteredProject>>,
) -> RwLockWriteGuard<'_, Vec<RegisteredProject>> {
    match lock.write() {
        Ok(projects) => projects,
        Err(poisoned) => poisoned.into_inner(),
    }
}
