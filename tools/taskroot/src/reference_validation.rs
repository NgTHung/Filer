//! # Reference Validation
//!
//! This module validates resolved parent and dependency edges before graph
//! consumers run. It shares the production resolver so validation and command
//! behavior cannot assign the same reference to different tasks.
//!
//! ```
//! use taskroot::identity::TaskReference;
//!
//! let reference = TaskReference::parse("core:CORE-001")?;
//! assert_eq!(reference.to_string(), "core:CORE-001");
//! # Ok::<(), taskroot::identity::IdentityError>(())
//! ```

use std::collections::{HashMap, HashSet};

use crate::{
    domain::validate_duplicate_identities,
    error::{TaskError, ValidationError},
    identity::{TaskIdentity, TaskReference},
    model::Task,
    project::TaskProject,
    reference::IdentityIndex,
    validate::ValidationWarning,
};

pub(crate) fn validate_task_references(
    project: &TaskProject,
    tasks: &[Task],
    warnings: &mut Vec<ValidationWarning>,
    errors: &mut Vec<ValidationError>,
) {
    validate_duplicate_identities(tasks, errors);
    let index = IdentityIndex::new(project.root(), tasks.iter().map(Task::identity));
    let mut dependencies: HashMap<TaskIdentity, Vec<TaskIdentity>> = HashMap::new();

    for task in tasks {
        if let Some(parent) = &task.metadata.parent {
            resolve_relationship(project, &index, task, "parent", parent, warnings, errors);
        }

        let mut seen_dependencies = HashSet::new();
        let mut seen_dependency_values = HashSet::new();
        for dependency in &task.metadata.depends_on {
            let duplicate_value = !seen_dependency_values.insert(dependency);
            let Some(identity) = resolve_relationship(
                project,
                &index,
                task,
                "dependency",
                dependency,
                warnings,
                errors,
            ) else {
                if duplicate_value {
                    errors.push(ValidationError::at(
                        &task.path,
                        format!("duplicate dependency reference {dependency}"),
                    ));
                }
                continue;
            };
            if !seen_dependencies.insert(identity.clone()) {
                errors.push(ValidationError::at(
                    &task.path,
                    format!("duplicate dependency {identity}"),
                ));
            }
            if identity == task.identity() {
                errors.push(ValidationError::at(
                    &task.path,
                    "task cannot depend on itself",
                ));
            }
            dependencies
                .entry(task.identity())
                .or_default()
                .push(identity);
        }
    }

    validate_dependency_cycles(tasks, &dependencies, errors);
}

fn resolve_relationship(
    project: &TaskProject,
    index: &IdentityIndex,
    task: &Task,
    kind: &str,
    value: &str,
    warnings: &mut Vec<ValidationWarning>,
    errors: &mut Vec<ValidationError>,
) -> Option<TaskIdentity> {
    let reference = match TaskReference::parse(value) {
        Ok(reference) => reference,
        Err(_) => return None,
    };
    match index.resolve_configured(
        &task.domain,
        &reference,
        project.policy().is_compatibility(),
    ) {
        Ok(resolved) => {
            if resolved.used_legacy_fallback {
                warnings.push(ValidationWarning {
                    code: "legacy_global_reference".to_string(),
                    path: Some(
                        task.path
                            .strip_prefix(project.root())
                            .unwrap_or(&task.path)
                            .to_string_lossy()
                            .replace('\\', "/"),
                    ),
                    message: format!(
                        "{kind} {value} resolved outside domain {} as {}",
                        task.domain, resolved.identity
                    ),
                    context: serde_json::json!({
                        "relationship": kind,
                        "reference": value,
                        "source_domain": task.domain,
                        "resolved_identity": resolved.identity.to_string(),
                        "project_root": project.root(),
                    }),
                });
            }
            Some(resolved.identity)
        }
        Err(error) => {
            let message = match &error {
                TaskError::TaskNotFound {
                    source_domain: Some(domain),
                    ..
                } => format!("{kind} {value} does not reference a task in domain {domain}"),
                TaskError::TaskNotFound { .. } => {
                    format!("{kind} {value} does not reference an existing task")
                }
                TaskError::AmbiguousReference { candidates, .. } => format!(
                    "{kind} {value} is ambiguous; candidates: {}",
                    candidates.join(", ")
                ),
                _ => error.to_string(),
            };
            errors.push(ValidationError::at(&task.path, message));
            None
        }
    }
}

fn validate_dependency_cycles(
    tasks: &[Task],
    dependencies: &HashMap<TaskIdentity, Vec<TaskIdentity>>,
    errors: &mut Vec<ValidationError>,
) {
    let mut checked = HashSet::new();

    for task in tasks {
        let mut visiting = Vec::new();
        detect_cycle(
            &task.identity(),
            dependencies,
            &mut visiting,
            &mut checked,
            errors,
        );
    }
}

fn detect_cycle(
    identity: &TaskIdentity,
    dependencies: &HashMap<TaskIdentity, Vec<TaskIdentity>>,
    visiting: &mut Vec<TaskIdentity>,
    checked: &mut HashSet<TaskIdentity>,
    errors: &mut Vec<ValidationError>,
) {
    if checked.contains(identity) {
        return;
    }

    if let Some(position) = visiting.iter().position(|current| current == identity) {
        let cycle = visiting[position..]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" -> ");
        errors.push(ValidationError::new(
            None,
            format!("dependency cycle detected: {cycle} -> {identity}"),
        ));
        return;
    }

    visiting.push(identity.clone());
    for dependency in dependencies.get(identity).into_iter().flatten() {
        detect_cycle(dependency, dependencies, visiting, checked, errors);
    }
    visiting.pop();
    checked.insert(identity.clone());
}
