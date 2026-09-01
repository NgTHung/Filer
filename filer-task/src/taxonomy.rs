//! # Task Taxonomy
//!
//! This module applies one project policy to task types, ID prefixes, tags,
//! criteria headings, and milestone roles. Keeping these checks together makes
//! stored-task validation and write preflight return the same reason codes.
//!
//! ```
//! use filer_task::{model::TaskType, project::TaskProject, taxonomy::criteria_heading};
//!
//! let root = tempfile::tempdir()?;
//! std::fs::create_dir(root.path().join(".tasks"))?;
//! let project = TaskProject::open(root.path())?;
//! let heading = criteria_heading(&project, &TaskType::new("Feature"), None, None)?;
//! assert_eq!(heading, "Acceptance Criteria");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::{
    error::{TaskError, TaxonomyErrorContext, ValidationError},
    identity::{is_windows_device_name, valid_hyphen_name},
    model::{Task, TaskType},
    project::{TagPolicy, TaskProject, TaskTypePolicy, TaskTypeRole},
};

pub fn task_type_policy<'a>(
    project: &'a TaskProject,
    task_type: &TaskType,
    domain: Option<&str>,
    task: Option<&str>,
) -> Result<&'a TaskTypePolicy, TaskError> {
    project
        .policy()
        .task_type(task_type.as_str())
        .ok_or_else(|| {
            TaskError::UnknownType(Box::new(TaxonomyErrorContext {
                rejected_value: task_type.to_string(),
                field: "type".to_string(),
                domain: domain.map(str::to_string),
                policy: None,
                allowed: project.policy().task_types().keys().cloned().collect(),
                project_root: project.root().to_path_buf(),
                task: task.map(str::to_string),
            }))
        })
}

pub fn criteria_heading(
    project: &TaskProject,
    task_type: &TaskType,
    domain: Option<&str>,
    task: Option<&str>,
) -> Result<&'static str, TaskError> {
    Ok(task_type_policy(project, task_type, domain, task)?
        .criteria()
        .heading())
}

pub fn is_milestone_type(project: &TaskProject, task_type: &TaskType) -> bool {
    project
        .policy()
        .task_type(task_type.as_str())
        .is_some_and(|policy| policy.role() == Some(TaskTypeRole::Milestone))
}

pub fn validate_prefix(
    project: &TaskProject,
    domain: &str,
    id: &str,
    task: Option<&str>,
) -> Result<(), TaskError> {
    let Some((prefix, _)) = id.split_once('-') else {
        return Ok(());
    };
    let Some(policy) = project.policy().domain(domain) else {
        return Ok(());
    };
    if policy.allows_prefix(prefix) {
        return Ok(());
    }
    Err(TaskError::PrefixNotAllowed(Box::new(
        TaxonomyErrorContext {
            rejected_value: prefix.to_string(),
            field: "id".to_string(),
            domain: Some(domain.to_string()),
            policy: None,
            allowed: policy.prefixes().to_vec(),
            project_root: project.root().to_path_buf(),
            task: task.map(str::to_string),
        },
    )))
}

pub fn validate_tag(
    project: &TaskProject,
    tag: &str,
    field: &str,
    domain: Option<&str>,
    task: Option<&str>,
) -> Result<(), TaskError> {
    let syntax_valid = valid_hyphen_name(tag, 64, false) && !is_windows_device_name(tag);
    let (policy_name, allowed, policy_allows) = match project.policy().tags() {
        TagPolicy::Open => ("open", Vec::new(), true),
        TagPolicy::Strict { allowed } => (
            "strict",
            allowed.clone(),
            allowed.iter().any(|value| value == tag),
        ),
    };
    if syntax_valid && policy_allows {
        return Ok(());
    }
    Err(TaskError::TagRejected(Box::new(TaxonomyErrorContext {
        rejected_value: tag.to_string(),
        field: field.to_string(),
        domain: domain.map(str::to_string),
        policy: Some(if syntax_valid {
            policy_name.to_string()
        } else {
            "portable-syntax".to_string()
        }),
        allowed,
        project_root: project.root().to_path_buf(),
        task: task.map(str::to_string),
    })))
}

pub fn validate_tags(
    project: &TaskProject,
    tags: &[String],
    field: &str,
    domain: Option<&str>,
    task: Option<&str>,
) -> Result<(), TaskError> {
    for tag in tags {
        validate_tag(project, tag, field, domain, task)?;
    }
    for (name, members) in project.policy().exclusive_tag_groups() {
        let selected: Vec<String> = tags
            .iter()
            .filter(|tag| members.contains(tag))
            .cloned()
            .collect();
        if selected.len() > 1 {
            return Err(TaskError::TagRejected(Box::new(TaxonomyErrorContext {
                rejected_value: selected.join(", "),
                field: field.to_string(),
                domain: domain.map(str::to_string),
                policy: Some(format!("exclusive group {name}")),
                allowed: members.clone(),
                project_root: project.root().to_path_buf(),
                task: task.map(str::to_string),
            })));
        }
    }
    Ok(())
}

pub(crate) fn validate_stored_task(
    project: &TaskProject,
    task: &Task,
    errors: &mut Vec<ValidationError>,
) {
    let identity = task.qualified_id();
    for result in [
        validate_prefix(project, &task.domain, &task.metadata.id, Some(&identity)),
        task_type_policy(
            project,
            &task.metadata.task_type,
            Some(&task.domain),
            Some(&identity),
        )
        .map(|_| ()),
        validate_tags(
            project,
            &task.metadata.tags,
            "tags",
            Some(&task.domain),
            Some(&identity),
        ),
    ] {
        if let Err(error) = result {
            errors.push(ValidationError::from_task_error(&task.path, &error));
        }
    }
}
