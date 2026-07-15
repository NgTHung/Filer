//! # Task Lifecycle
//!
//! This module owns task file mutations for lifecycle commands. CLI handlers pass
//! command intent here, so file editing rules stay testable without terminal output.
//!
//! ```
//! use filer_task::model::{Priority, TaskType};
//!
//! assert_eq!(Priority::High.to_string(), "High");
//! assert_eq!(TaskType::new("Feature").to_string(), "Feature");
//! ```

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    error::TaskError,
    frontmatter::{parse_metadata, render_metadata},
    identity::{IdentityError, TaskIdentity, TaskReference},
    markdown::has_unchecked_checklist_item,
    model::{Priority, Risk, TaskStatus, TaskType},
    project::TaskProject,
    reference::IdentityIndex,
    repo::TASK_DIR,
    taxonomy::{
        criteria_heading, is_milestone_type, task_type_policy, validate_prefix, validate_tags,
    },
    validate::{RULE_IDS, require_valid_report, validate_repo},
};

use self::write::{read, today, write_new_task, write_status};

mod criteria;
mod edit;
mod write;

pub use criteria::{set_criterion_checked, toggle_criterion};
pub use edit::{FieldPatch, TaskPatch, edit_task};

#[derive(Debug, Clone, Deserialize)]
pub struct Criterion {
    pub text: String,
    #[serde(default)]
    pub checked: bool,
}

#[derive(Clone)]
pub struct NewTask {
    pub domain: String,
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: Priority,
    pub task_type: TaskType,
    pub parent: Option<String>,
    pub milestone: Option<String>,
    pub depends_on: Vec<String>,
    pub rules: Vec<String>,
    pub risk: Option<Risk>,
    pub impact: Option<String>,
    pub tags: Vec<String>,
    pub whitepaper: Option<String>,
    pub summary: Option<String>,
    pub criteria: Vec<Criterion>,
    pub rationale: Option<String>,
    pub blocked_reason: Option<String>,
}

pub fn add_task(project: &TaskProject, mut task: NewTask) -> Result<PathBuf, TaskError> {
    project.with_write_lock(|| {
        validate_new_tasks(project, std::slice::from_mut(&mut task), false)?;
        let (path, content) = render_task(project, &task)?;
        write_new_task(&path, &content, &format!("{}:{}", task.domain, task.id))?;
        Ok(path)
    })
}

pub fn import_tasks(
    project: &TaskProject,
    tasks: &[NewTask],
    dry_run: bool,
    skip_existing: bool,
) -> Result<Vec<PathBuf>, TaskError> {
    if dry_run {
        return import_tasks_unlocked(project, tasks, true, skip_existing);
    }
    project.with_write_lock(|| import_tasks_unlocked(project, tasks, false, skip_existing))
}

fn import_tasks_unlocked(
    project: &TaskProject,
    tasks: &[NewTask],
    dry_run: bool,
    skip_existing: bool,
) -> Result<Vec<PathBuf>, TaskError> {
    let existing = existing_identities(project)?;
    let mut tasks_to_write: Vec<NewTask> = tasks
        .iter()
        .filter(|task| {
            !skip_existing
                || !TaskIdentity::new(&task.domain, &task.id)
                    .is_ok_and(|identity| existing.contains(&identity))
        })
        .cloned()
        .collect();
    validate_new_tasks(project, &mut tasks_to_write, skip_existing)?;
    let rendered: Vec<(PathBuf, String)> = tasks_to_write
        .iter()
        .map(|task| render_task(project, task))
        .collect::<Result<_, _>>()?;
    if dry_run {
        return Ok(rendered.into_iter().map(|(path, _)| path).collect());
    }
    for (task, (path, content)) in tasks_to_write.iter().zip(&rendered) {
        write_new_task(path, content, &format!("{}:{}", task.domain, task.id))?;
    }
    Ok(rendered.into_iter().map(|(path, _)| path).collect())
}

fn validate_new_tasks(
    project: &TaskProject,
    new_tasks: &mut [NewTask],
    skip_existing: bool,
) -> Result<(), TaskError> {
    let existing_tasks = require_valid_report(validate_repo(project)?)?;
    let mut known_identities: HashSet<TaskIdentity> = existing_tasks
        .iter()
        .map(|task| task_identity(project, &task.domain, &task.metadata.id))
        .collect::<Result<_, _>>()?;
    let mut milestone_counts: HashMap<String, usize> = HashMap::new();
    for task in &existing_tasks {
        if is_milestone_type(project, &task.metadata.task_type)
            && let Some(milestone) = &task.metadata.milestone
        {
            *milestone_counts.entry(milestone.clone()).or_default() += 1;
        }
    }

    let mut batch_ids = HashSet::new();
    for task in new_tasks.iter() {
        let identity = validate_new_task_shape(project, task)?;
        let path = new_task_path(project.root(), task);
        if path.exists() && !skip_existing {
            return Err(TaskError::IdExists {
                task: identity.to_string(),
                path,
            });
        }
        if known_identities.contains(&identity) && !skip_existing {
            return Err(TaskError::IdExists {
                task: identity.to_string(),
                path,
            });
        }
        if !batch_ids.insert(identity.clone()) {
            return Err(TaskError::Message(format!(
                "task {identity} appears more than once in import"
            )));
        }
        known_identities.insert(identity);
        if is_milestone_type(project, &task.task_type) {
            let milestone = task
                .milestone
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    TaskError::Message(
                        "milestone-role tasks must include a non-empty milestone value".to_string(),
                    )
                })?;
            *milestone_counts.entry(milestone.to_string()).or_default() += 1;
        }
    }

    let index = IdentityIndex::new(project.root(), known_identities.iter().cloned());
    let batch_identities: HashSet<TaskIdentity> = new_tasks
        .iter()
        .map(|task| task_identity(project, &task.domain, &task.id))
        .collect::<Result<_, _>>()?;
    let mut batch_dependencies: HashMap<TaskIdentity, Vec<TaskIdentity>> = HashMap::new();
    for task in new_tasks.iter_mut() {
        let source = task_identity(project, &task.domain, &task.id)?;
        if let Some(parent) = &mut task.parent {
            resolve_new_reference(project, &index, &task.domain, parent)?;
        }
        let mut seen_dependencies = HashSet::new();
        for dependency in &mut task.depends_on {
            let identity = resolve_new_reference(project, &index, &task.domain, dependency)?;
            if !seen_dependencies.insert(identity.clone()) {
                return Err(TaskError::Message(format!(
                    "duplicate dependency {identity}"
                )));
            }
            if identity == source {
                return Err(TaskError::Message(
                    "task cannot depend on itself".to_string(),
                ));
            }
            if batch_identities.contains(&identity) {
                batch_dependencies
                    .entry(source.clone())
                    .or_default()
                    .push(identity);
            }
        }
        if let Some(milestone) = &task.milestone {
            let count = milestone_counts
                .get(milestone.as_str())
                .copied()
                .unwrap_or(0);
            if count != 1 {
                return Err(TaskError::Message(format!(
                    "milestone {milestone} must reference exactly one milestone task"
                )));
            }
        }
    }
    validate_new_dependency_cycles(&batch_identities, &batch_dependencies)?;

    Ok(())
}

fn validate_new_task_shape(
    project: &TaskProject,
    task: &NewTask,
) -> Result<TaskIdentity, TaskError> {
    let identity = task_identity(project, &task.domain, &task.id)?;
    if task.title.chars().count() < 5 {
        return Err(TaskError::Message(format!(
            "{} title must be at least 5 characters",
            task.id
        )));
    }
    if project.policy().domain(&task.domain).is_none() {
        return Err(TaskError::UnknownDomain {
            domain: task.domain.clone(),
            configured: project.policy().domains().keys().cloned().collect(),
            root: project.root().to_path_buf(),
        });
    }
    let task_name = identity.to_string();
    task_type_policy(
        project,
        &task.task_type,
        Some(&task.domain),
        Some(&task_name),
    )?;
    validate_prefix(project, &task.domain, &task.id, Some(&task_name))?;
    validate_tags(
        project,
        &task.tags,
        "tags",
        Some(&task.domain),
        Some(&task_name),
    )?;
    let mut seen_rules = HashSet::new();
    for rule in &task.rules {
        if !RULE_IDS.contains(&rule.as_str()) {
            return Err(TaskError::Message(format!("unknown rule id {rule}")));
        }
        if !seen_rules.insert(rule) {
            return Err(TaskError::Message(format!("duplicate rule id {rule}")));
        }
    }
    if let Some(impact) = &task.impact
        && impact.chars().count() < 10
    {
        return Err(TaskError::Message(
            "impact must be at least 10 characters when present".to_string(),
        ));
    }
    if task.status == TaskStatus::Blocked && task.blocked_reason.is_none() {
        return Err(TaskError::Message(format!(
            "{} uses Blocked status and must include blocked_reason",
            task.id
        )));
    }
    if matches!(task.status, TaskStatus::Deferred | TaskStatus::Obsolete)
        && task.rationale.is_none()
    {
        return Err(TaskError::Message(format!(
            "{} uses {} status and must include rationale",
            task.id, task.status
        )));
    }
    if task.status == TaskStatus::Done
        && (task.criteria.is_empty() || task.criteria.iter().any(|criterion| !criterion.checked))
    {
        return Err(TaskError::Message(format!(
            "{} cannot be created Done with unchecked criteria",
            task.id
        )));
    }
    if is_milestone_type(project, &task.task_type)
        && task
            .milestone
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(TaskError::Message(
            "milestone-role tasks must include a non-empty milestone value".to_string(),
        ));
    }
    Ok(identity)
}

fn validate_new_dependency_cycles(
    identities: &HashSet<TaskIdentity>,
    dependencies: &HashMap<TaskIdentity, Vec<TaskIdentity>>,
) -> Result<(), TaskError> {
    let mut checked = HashSet::new();
    let mut identities = identities.iter().collect::<Vec<_>>();
    identities.sort();
    for identity in identities {
        let mut visiting = Vec::new();
        detect_new_cycle(identity, dependencies, &mut visiting, &mut checked)?;
    }
    Ok(())
}

fn detect_new_cycle(
    identity: &TaskIdentity,
    dependencies: &HashMap<TaskIdentity, Vec<TaskIdentity>>,
    visiting: &mut Vec<TaskIdentity>,
    checked: &mut HashSet<TaskIdentity>,
) -> Result<(), TaskError> {
    if checked.contains(identity) {
        return Ok(());
    }
    if let Some(position) = visiting.iter().position(|current| current == identity) {
        let cycle = visiting[position..]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(TaskError::Message(format!(
            "dependency cycle detected: {cycle} -> {identity}"
        )));
    }
    visiting.push(identity.clone());
    for dependency in dependencies.get(identity).into_iter().flatten() {
        detect_new_cycle(dependency, dependencies, visiting, checked)?;
    }
    visiting.pop();
    checked.insert(identity.clone());
    Ok(())
}

fn resolve_new_reference(
    project: &TaskProject,
    index: &IdentityIndex,
    source_domain: &str,
    value: &mut String,
) -> Result<TaskIdentity, TaskError> {
    let reference =
        TaskReference::parse(value).map_err(|error| invalid_identity(project, &error))?;
    let identity = index.resolve_creation(source_domain, &reference)?;
    if matches!(reference, TaskReference::Qualified(_)) {
        *value = identity.to_string();
    }
    Ok(identity)
}

fn new_task_path(root: &Path, task: &NewTask) -> PathBuf {
    let relative = format!("{}-{}.md", task.id, slug(&task.title));
    root.join(TASK_DIR).join(&task.domain).join(relative)
}

fn render_task(project: &TaskProject, task: &NewTask) -> Result<(PathBuf, String), TaskError> {
    let path = new_task_path(project.root(), task);
    let content = render_new_task(project, task)?;
    Ok((path, content))
}

fn render_new_task(project: &TaskProject, task: &NewTask) -> Result<String, TaskError> {
    let metadata = crate::model::TaskMetadata {
        id: task.id.clone(),
        title: task.title.clone(),
        status: task.status,
        priority: task.priority,
        task_type: task.task_type.clone(),
        parent: task.parent.clone(),
        milestone: task.milestone.clone(),
        depends_on: task.depends_on.clone(),
        rules: task.rules.clone(),
        risk: task.risk,
        impact: task.impact.clone(),
        tags: task.tags.clone(),
        whitepaper: task.whitepaper.clone(),
        last_updated: Some(today()),
    };
    let mut content = render_metadata(&metadata)?;
    content.push('\n');

    content.push_str("## Summary\n\n");
    content.push_str(
        task.summary
            .as_deref()
            .unwrap_or("Describe why this work exists."),
    );
    content.push_str("\n\n");

    if let Some(reason) = &task.blocked_reason {
        content.push_str("## Blocked Reason\n\n");
        content.push_str(reason.trim());
        content.push_str("\n\n");
    }
    if let Some(rationale) = &task.rationale {
        content.push_str("## Rationale\n\n");
        content.push_str(rationale.trim());
        content.push_str("\n\n");
        return Ok(content);
    }

    let task_name = format!("{}:{}", task.domain, task.id);
    let criteria_heading = criteria_heading(
        project,
        &task.task_type,
        Some(&task.domain),
        Some(&task_name),
    )?;
    content.push_str(&format!("## {criteria_heading}\n\n"));
    if task.criteria.is_empty() {
        content.push_str("- [ ] Define completion criteria.\n");
    } else {
        for criterion in &task.criteria {
            let marker = if criterion.checked { "x" } else { " " };
            content.push_str(&format!("- [{marker}] {}\n", criterion.text.trim()));
        }
    }

    Ok(content)
}

fn existing_identities(project: &TaskProject) -> Result<HashSet<TaskIdentity>, TaskError> {
    require_valid_report(validate_repo(project)?)?
        .into_iter()
        .map(|task| task_identity(project, &task.domain, &task.metadata.id))
        .collect()
}

fn task_identity(
    project: &TaskProject,
    domain: &str,
    local_id: &str,
) -> Result<TaskIdentity, TaskError> {
    TaskIdentity::new(domain, local_id).map_err(|error| invalid_identity(project, &error))
}

fn invalid_identity(project: &TaskProject, error: &IdentityError) -> TaskError {
    TaskError::InvalidReference {
        reference: error.value().to_string(),
        constraint: error.constraint().to_string(),
        root: project.root().to_path_buf(),
    }
}

pub fn start_task(project: &TaskProject, identity: &TaskIdentity) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project.with_write_lock(|| update_status(&path, TaskStatus::InProgress, None))
}

pub fn block_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    reason: &str,
) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project.with_write_lock(|| {
        update_status(&path, TaskStatus::Blocked, Some(("Blocked Reason", reason)))
    })
}

pub fn defer_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    reason: &str,
) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project
        .with_write_lock(|| update_status(&path, TaskStatus::Deferred, Some(("Rationale", reason))))
}

pub fn obsolete_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    reason: &str,
) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project
        .with_write_lock(|| update_status(&path, TaskStatus::Obsolete, Some(("Rationale", reason))))
}

pub fn done_task(project: &TaskProject, identity: &TaskIdentity) -> Result<PathBuf, TaskError> {
    let path = project.task_path(identity)?;
    project.with_write_lock(|| done_task_unlocked(project, identity, &path))
}

fn done_task_unlocked(
    project: &TaskProject,
    identity: &TaskIdentity,
    path: &Path,
) -> Result<PathBuf, TaskError> {
    let content = read(path)?;
    let metadata =
        parse_metadata(path, &content).map_err(|error| TaskError::Validation(vec![error]))?;
    let heading = criteria_heading(
        project,
        &metadata.task_type,
        Some(&identity.domain),
        Some(&identity.to_string()),
    )?;
    if has_unchecked_checklist_item(&content, heading) {
        return Err(TaskError::Message(format!(
            "{identity} cannot be marked Done while ## {heading} has unchecked items"
        )));
    }
    write_status(path, &content, TaskStatus::Done, None)?;
    Ok(path.to_path_buf())
}

fn update_status(
    path: &Path,
    status: TaskStatus,
    section: Option<(&str, &str)>,
) -> Result<PathBuf, TaskError> {
    let content = read(path)?;
    write_status(path, &content, status, section)?;
    Ok(path.to_path_buf())
}

fn slug(title: &str) -> String {
    let mut slug = String::new();
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}
