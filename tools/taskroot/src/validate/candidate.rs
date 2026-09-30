//! # Candidate Validation
//!
//! This module validates in-memory task and policy candidates against unchanged
//! files in the real project. Target validation keeps local edits cheap, while
//! repository validation checks relationships and policy effects in one pass.
//!
//! ```
//! use taskroot::{project::TaskProject, validate::validate_repo};
//!
//! # let root = tempfile::tempdir()?;
//! # std::fs::create_dir(root.path().join(".tasks"))?;
//! let project = TaskProject::open(root.path())?;
//! assert!(validate_repo(&project)?.errors.is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{fs, path::Path};

use crate::{
    error::{TaskError, ValidationError},
    frontmatter::parse_metadata,
    model::Task,
    project::{ProjectPolicy, TaskProject},
    reference_validation::validate_task_references,
    repo::{read_task_files, validate_task_layout},
};

use super::{
    ValidationReport, domain_for_path, validate_body, validate_milestone_references,
    validate_single_task,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct CandidateOverrides<'a> {
    pub(crate) task: Option<TaskContentOverride<'a>>,
    pub(crate) policy: Option<&'a ProjectPolicy>,
}

#[derive(Clone, Copy)]
pub(crate) struct TaskContentOverride<'a> {
    pub(crate) path: &'a Path,
    pub(crate) content: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) enum CandidateScope {
    Target,
    Repository,
}

pub(crate) fn validate_candidate(
    project: &TaskProject,
    overrides: CandidateOverrides<'_>,
    scope: CandidateScope,
) -> Result<ValidationReport, TaskError> {
    let candidate_project = overrides
        .policy
        .map(|policy| project.with_candidate_policy(policy.clone()));
    let active_project = candidate_project.as_ref().unwrap_or(project);

    match scope {
        CandidateScope::Target => validate_target(active_project, overrides.task),
        CandidateScope::Repository => validate_repository(active_project, overrides.task),
    }
}

fn validate_target(
    project: &TaskProject,
    task_override: Option<TaskContentOverride<'_>>,
) -> Result<ValidationReport, TaskError> {
    let Some(task_override) = task_override else {
        return Err(TaskError::Message(
            "target candidate validation requires task content".to_string(),
        ));
    };
    let mut errors = Vec::new();
    let mut tasks = Vec::new();
    load_and_validate_task(
        project,
        task_override.path,
        task_override.content,
        &mut tasks,
        &mut errors,
    );
    Ok(ValidationReport {
        tasks,
        errors,
        warnings: Vec::new(),
    })
}

fn validate_repository(
    project: &TaskProject,
    task_override: Option<TaskContentOverride<'_>>,
) -> Result<ValidationReport, TaskError> {
    let paths = read_task_files(project)?;
    let mut tasks = Vec::new();
    let mut errors = validate_task_layout(project)?;
    let mut warnings = Vec::new();

    for path in paths {
        let stored;
        let content = if let Some(task_override) = task_override.filter(|item| item.path == path) {
            task_override.content
        } else {
            stored = fs::read_to_string(&path).map_err(|source| TaskError::Io {
                path: path.clone(),
                source,
            })?;
            &stored
        };
        load_and_validate_task(project, &path, content, &mut tasks, &mut errors);
    }

    validate_task_references(project, &tasks, &mut warnings, &mut errors);
    validate_milestone_references(project, &tasks, &mut errors);
    tasks.sort_by(|left, right| {
        left.metadata
            .id
            .cmp(&right.metadata.id)
            .then_with(|| left.domain.cmp(&right.domain))
    });

    Ok(ValidationReport {
        tasks,
        errors,
        warnings,
    })
}

fn load_and_validate_task(
    project: &TaskProject,
    path: &Path,
    content: &str,
    tasks: &mut Vec<Task>,
    errors: &mut Vec<ValidationError>,
) {
    match parse_metadata(path, content) {
        Ok(metadata) => {
            let domain =
                domain_for_path(project.root(), path).unwrap_or_else(|| "unknown".to_string());
            let task = Task {
                path: path.to_path_buf(),
                domain,
                metadata,
            };
            validate_single_task(project, &task, errors);
            validate_body(project, &task, content, errors);
            tasks.push(task);
        }
        Err(error) => errors.push(error),
    }
}
