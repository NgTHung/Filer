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
    error::ValidationError,
    project::TaskProject,
    repo::discover_project_root,
    validate::{ValidatedRepository, validate_repo},
};
use tokio::sync::Mutex;

use crate::{
    dto::{ProjectSummary, ValidationIssue},
    error::WebError,
    storage::ProjectRegistration,
};

#[derive(Debug, Clone)]
pub struct RegisteredProject {
    name: String,
    root: PathBuf,
    task_project: Option<TaskProject>,
    open_issue: Option<ValidationIssue>,
    write_lock: Arc<Mutex<()>>,
}

impl RegisteredProject {
    pub(crate) fn new(task_project: TaskProject) -> Result<Self, WebError> {
        let name = project_name(&task_project)?;
        let root = task_project.root().to_path_buf();
        Ok(Self {
            name,
            root,
            task_project: Some(task_project),
            open_issue: None,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

    fn reopen(registration: ProjectRegistration) -> Self {
        match TaskProject::open(&registration.root) {
            Ok(task_project) => Self {
                name: registration.name,
                root: registration.root,
                task_project: Some(task_project),
                open_issue: None,
                write_lock: Arc::new(Mutex::new(())),
            },
            Err(error) => Self {
                name: registration.name,
                open_issue: Some(
                    ValidationError::from_task_error(&registration.root, &error).into(),
                ),
                root: registration.root,
                task_project: None,
                write_lock: Arc::new(Mutex::new(())),
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn task_project(&self) -> Result<&TaskProject, WebError> {
        self.task_project
            .as_ref()
            .ok_or_else(|| self.broken_error())
    }

    pub fn registration(&self) -> ProjectRegistration {
        ProjectRegistration::new(self.name.clone(), self.root.clone())
    }

    pub fn write_lock(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.write_lock)
    }

    pub fn validate(&self) -> Result<ValidatedRepository, WebError> {
        self.validate_project(self.task_project()?)
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
        let Some(task_project) = &self.task_project else {
            return Ok(self.unavailable_summary());
        };
        let report = match validate_repo(task_project) {
            Ok(report) => report,
            Err(error) => {
                return Ok(ProjectSummary {
                    name: self.name.clone(),
                    task_count: 0,
                    domain_count: 0,
                    broken: true,
                    issues: vec![ValidationError::from_task_error(&self.root, &error).into()],
                });
            }
        };
        let issues: Vec<_> = report
            .errors
            .into_iter()
            .map(ValidationIssue::from)
            .collect();
        Ok(ProjectSummary {
            name: self.name.clone(),
            task_count: report.tasks.len(),
            domain_count: task_project.policy().domains().len(),
            broken: !issues.is_empty(),
            issues,
        })
    }

    fn unavailable_summary(&self) -> ProjectSummary {
        ProjectSummary {
            name: self.name.clone(),
            task_count: 0,
            domain_count: 0,
            broken: true,
            issues: self.open_issue.clone().into_iter().collect(),
        }
    }

    fn broken_error(&self) -> WebError {
        WebError::ProjectBroken {
            name: self.name.clone(),
            issues: self.open_issue.clone().into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectRegistry {
    projects: Arc<RwLock<Vec<RegisteredProject>>>,
}

impl ProjectRegistry {
    pub fn from_registrations(registrations: Vec<ProjectRegistration>) -> Self {
        Self {
            projects: Arc::new(RwLock::new(
                registrations
                    .into_iter()
                    .map(RegisteredProject::reopen)
                    .collect(),
            )),
        }
    }

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

    pub(crate) fn ensure_name_available(&self, name: &str) -> Result<(), WebError> {
        if read_projects(&self.projects)
            .iter()
            .any(|project| project.name == name)
        {
            return Err(WebError::DuplicateProjectName(name.to_string()));
        }
        Ok(())
    }

    pub(crate) fn publish(&self, registered: RegisteredProject) {
        let mut projects = write_projects(&self.projects);
        projects.push(registered);
    }

    pub(crate) fn remove(&self, name: &str) {
        let mut projects = write_projects(&self.projects);
        projects.retain(|project| project.name != name);
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
        if registered.root != task_project.root() {
            return Err(WebError::BadRequest(format!(
                "replacement root {} does not match registered root {}",
                task_project.root().display(),
                registered.root.display()
            )));
        }
        registered.task_project = Some(task_project);
        registered.open_issue = None;
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
