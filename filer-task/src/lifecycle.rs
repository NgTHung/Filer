//! # Task Lifecycle
//!
//! This module owns task file mutations for lifecycle commands. CLI handlers pass
//! command intent here, so file editing rules stay testable without terminal output.
//!
//! ```
//! use filer_task::model::{Priority, TaskType};
//!
//! assert_eq!(Priority::High.to_string(), "High");
//! assert_eq!(TaskType::Feature.to_string(), "Feature");
//! ```

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;

use crate::{
    domain::compatibility_prefixes,
    error::TaskError,
    frontmatter::parse_metadata,
    identity::{IdentityError, TaskIdentity, TaskReference},
    markdown::{has_unchecked_checklist_item, replace_or_append_section},
    model::{Priority, Risk, TaskStatus, TaskType},
    project::TaskProject,
    reference::IdentityIndex,
    repo::{MILESTONE_DOMAIN, TASK_DIR},
    validate::{RULE_IDS, require_valid_report, validate_repo},
};

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
    validate_new_tasks(project, std::slice::from_mut(&mut task), false)?;
    let (path, content) = render_task(project.root(), &task);
    write_new_task(&path, &content)?;
    Ok(path)
}

pub fn import_tasks(
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
        .map(|task| render_task(project.root(), task))
        .collect();
    if dry_run {
        return Ok(rendered.into_iter().map(|(path, _)| path).collect());
    }
    for (path, content) in &rendered {
        write_new_task(path, content)?;
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
        if task.metadata.task_type == TaskType::Milestone {
            if let Some(milestone) = &task.metadata.milestone {
                *milestone_counts.entry(milestone.clone()).or_default() += 1;
            }
        }
    }

    let mut batch_ids = HashSet::new();
    for task in new_tasks.iter() {
        let identity = validate_new_task_shape(project, task)?;
        let (path, _) = render_task(project.root(), task);
        if path.exists() && !skip_existing {
            return Err(TaskError::Message(format!(
                "task file already exists: {}",
                path.display()
            )));
        }
        if known_identities.contains(&identity) && !skip_existing {
            return Err(TaskError::Message(format!(
                "task {identity} already exists"
            )));
        }
        if !batch_ids.insert(identity.clone()) {
            return Err(TaskError::Message(format!(
                "task {identity} appears more than once in import"
            )));
        }
        known_identities.insert(identity);
        if task.task_type == TaskType::Milestone {
            let milestone = task.milestone.as_deref().ok_or_else(|| {
                TaskError::Message("Milestone tasks must include --milestone".to_string())
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
    if task.task_type == TaskType::Milestone && task.domain != MILESTONE_DOMAIN {
        return Err(TaskError::Message(
            "Milestone tasks must be created under .tasks/milestones".to_string(),
        ));
    }
    if task.task_type != TaskType::Milestone && task.domain == MILESTONE_DOMAIN {
        return Err(TaskError::Message(
            ".tasks/milestones only accepts Milestone tasks".to_string(),
        ));
    }
    let prefix = task
        .id
        .split_once('-')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
    if project.policy().is_compatibility()
        && prefix == "MILESTONE"
        && task.domain != MILESTONE_DOMAIN
    {
        return Err(TaskError::Message(
            "MILESTONE prefix is only allowed under .tasks/milestones".to_string(),
        ));
    }
    if project.policy().is_compatibility()
        && !compatibility_prefixes(&task.domain).contains(&prefix)
    {
        return Err(TaskError::Message(format!(
            "prefix {prefix} is not allowed for {} tasks",
            task.domain
        )));
    }
    let mut seen_rules = HashSet::new();
    for rule in &task.rules {
        if !RULE_IDS.contains(&rule.as_str()) {
            return Err(TaskError::Message(format!("unknown rule id {rule}")));
        }
        if !seen_rules.insert(rule) {
            return Err(TaskError::Message(format!("duplicate rule id {rule}")));
        }
    }
    if let Some(impact) = &task.impact {
        if impact.chars().count() < 10 {
            return Err(TaskError::Message(
                "impact must be at least 10 characters when present".to_string(),
            ));
        }
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
    if task.task_type == TaskType::Milestone && task.milestone.is_none() {
        return Err(TaskError::Message(
            "Milestone tasks must include --milestone".to_string(),
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

fn render_task(root: &Path, task: &NewTask) -> (PathBuf, String) {
    let relative = format!("{}-{}.md", task.id, slug(&task.title));
    let path = root.join(TASK_DIR).join(&task.domain).join(relative);
    let content = render_new_task(task);
    (path, content)
}

fn render_new_task(task: &NewTask) -> String {
    let mut content = format!(
        "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\ntype: {task_type}\n",
        id = task.id,
        title = task.title,
        status = task.status,
        priority = task.priority,
        task_type = task.task_type
    );
    push_optional_scalar(&mut content, "parent", task.parent.as_deref(), false);
    push_optional_scalar(&mut content, "milestone", task.milestone.as_deref(), true);
    push_array(&mut content, "depends_on", &task.depends_on);
    push_array(&mut content, "rules", &task.rules);
    if let Some(risk) = task.risk {
        content.push_str(&format!("risk: {risk}\n"));
    }
    push_optional_scalar(&mut content, "impact", task.impact.as_deref(), false);
    push_array(&mut content, "tags", &task.tags);
    push_optional_scalar(
        &mut content,
        "whitepaper",
        task.whitepaper.as_deref(),
        false,
    );
    content.push_str(&format!("last_updated: {}\n---\n\n", today()));

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
        return content;
    }

    let criteria_heading = if matches!(task.task_type, TaskType::Milestone | TaskType::Epic) {
        "Exit Criteria"
    } else {
        "Acceptance Criteria"
    };
    content.push_str(&format!("## {criteria_heading}\n\n"));
    if task.criteria.is_empty() {
        content.push_str("- [ ] Define completion criteria.\n");
    } else {
        for criterion in &task.criteria {
            let marker = if criterion.checked { "x" } else { " " };
            content.push_str(&format!("- [{marker}] {}\n", criterion.text.trim()));
        }
    }

    content
}

fn push_optional_scalar(content: &mut String, key: &str, value: Option<&str>, quoted: bool) {
    if let Some(value) = value {
        if quoted {
            content.push_str(&format!("{key}: \"{}\"\n", escape_yaml(value)));
        } else {
            content.push_str(&format!("{key}: {}\n", escape_yaml(value)));
        }
    }
}

fn push_array(content: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    content.push_str(&format!("{key}: ["));
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            content.push_str(", ");
        }
        content.push_str(&escape_yaml(value));
    }
    content.push_str("]\n");
}

fn escape_yaml(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':'))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
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

fn write_new_task(path: &Path, content: &str) -> Result<(), TaskError> {
    if path.exists() {
        return Err(TaskError::Message(format!(
            "task file already exists: {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn start_task(project: &TaskProject, identity: &TaskIdentity) -> Result<PathBuf, TaskError> {
    update_status(project, identity, TaskStatus::InProgress, None)
}

pub fn block_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    reason: &str,
) -> Result<PathBuf, TaskError> {
    update_status(
        project,
        identity,
        TaskStatus::Blocked,
        Some(("Blocked Reason", reason)),
    )
}

pub fn defer_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    reason: &str,
) -> Result<PathBuf, TaskError> {
    update_status(
        project,
        identity,
        TaskStatus::Deferred,
        Some(("Rationale", reason)),
    )
}

pub fn obsolete_task(
    project: &TaskProject,
    identity: &TaskIdentity,
    reason: &str,
) -> Result<PathBuf, TaskError> {
    update_status(
        project,
        identity,
        TaskStatus::Obsolete,
        Some(("Rationale", reason)),
    )
}

pub fn done_task(project: &TaskProject, identity: &TaskIdentity) -> Result<PathBuf, TaskError> {
    let path = task_path(project, identity)?;
    let content = read(&path)?;
    let metadata =
        parse_metadata(&path, &content).map_err(|error| TaskError::Validation(vec![error]))?;
    let heading = if matches!(metadata.task_type, TaskType::Milestone | TaskType::Epic) {
        "Exit Criteria"
    } else {
        "Acceptance Criteria"
    };
    if has_unchecked_checklist_item(&content, heading) {
        return Err(TaskError::Message(format!(
            "{identity} cannot be marked Done while ## {heading} has unchecked items"
        )));
    }
    write_status(&path, &content, TaskStatus::Done, None)?;
    Ok(path)
}

fn update_status(
    project: &TaskProject,
    identity: &TaskIdentity,
    status: TaskStatus,
    section: Option<(&str, &str)>,
) -> Result<PathBuf, TaskError> {
    let path = task_path(project, identity)?;
    let content = read(&path)?;
    write_status(&path, &content, status, section)?;
    Ok(path)
}

fn task_path(project: &TaskProject, identity: &TaskIdentity) -> Result<PathBuf, TaskError> {
    let tasks = require_valid_report(validate_repo(project)?)?;
    tasks
        .into_iter()
        .find(|task| task.identity() == *identity)
        .map(|task| task.path)
        .ok_or_else(|| TaskError::TaskNotFound {
            reference: identity.to_string(),
            source_domain: None,
            root: project.root().to_path_buf(),
        })
}

fn write_status(
    path: &Path,
    content: &str,
    status: TaskStatus,
    section: Option<(&str, &str)>,
) -> Result<(), TaskError> {
    let mut updated = update_frontmatter(content, status);
    if let Some((heading, text)) = section {
        updated = replace_or_append_section(&updated, heading, text);
    }
    fs::write(path, updated).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn update_frontmatter(content: &str, status: TaskStatus) -> String {
    let mut in_frontmatter = false;
    let mut seen_status = false;
    let mut seen_last_updated = false;
    let mut output = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if index == 0 && line.trim() == "---" {
            in_frontmatter = true;
            output.push(line.to_string());
            continue;
        }
        if in_frontmatter && line.trim() == "---" {
            if !seen_status {
                output.push(format!("status: {status}"));
            }
            if !seen_last_updated {
                output.push(format!("last_updated: {}", today()));
            }
            in_frontmatter = false;
            output.push(line.to_string());
            continue;
        }
        if in_frontmatter && line.starts_with("status:") {
            output.push(format!("status: {status}"));
            seen_status = true;
        } else if in_frontmatter && line.starts_with("last_updated:") {
            output.push(format!("last_updated: {}", today()));
            seen_last_updated = true;
        } else {
            output.push(line.to_string());
        }
    }

    let mut joined = output.join("\n");
    joined.push('\n');
    joined
}

fn read(path: &Path) -> Result<String, TaskError> {
    fs::read_to_string(path).map_err(|source| TaskError::Io {
        path: path.to_path_buf(),
        source,
    })
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

fn today() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}
