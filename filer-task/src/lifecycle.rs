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
    error::TaskError,
    frontmatter::parse_metadata,
    markdown::{has_unchecked_checklist_item, replace_or_append_section},
    model::{Priority, Risk, TaskStatus, TaskType},
    repo::{DOMAINS, MILESTONE_DOMAIN, TASK_DIR},
    validate::{RULE_IDS, allowed_prefixes, is_valid_task_id, require_valid_report, validate_repo},
};

#[derive(Debug, Clone, Deserialize)]
pub struct Criterion {
    pub text: String,
    #[serde(default)]
    pub checked: bool,
}

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

pub fn add_task(root: &Path, task: NewTask) -> Result<PathBuf, TaskError> {
    validate_new_tasks(root, &[&task], false)?;
    let (path, content) = render_task(root, &task);
    write_new_task(&path, &content)?;
    Ok(path)
}

pub fn import_tasks(
    root: &Path,
    tasks: &[NewTask],
    dry_run: bool,
    skip_existing: bool,
) -> Result<Vec<PathBuf>, TaskError> {
    let existing = existing_ids(root)?;
    let tasks_to_write: Vec<&NewTask> = tasks
        .iter()
        .filter(|task| !skip_existing || !existing.contains(task.id.as_str()))
        .collect();
    validate_new_tasks(root, &tasks_to_write, skip_existing)?;
    let rendered: Vec<(PathBuf, String)> = tasks_to_write
        .iter()
        .map(|task| render_task(root, task))
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
    root: &Path,
    new_tasks: &[&NewTask],
    skip_existing: bool,
) -> Result<(), TaskError> {
    let existing_tasks = require_valid_report(validate_repo(root)?)?;
    let mut known_ids: HashSet<&str> = existing_tasks
        .iter()
        .map(|task| task.metadata.id.as_str())
        .collect();
    let mut milestone_counts: HashMap<&str, usize> = HashMap::new();
    for task in &existing_tasks {
        if task.metadata.task_type == TaskType::Milestone {
            if let Some(milestone) = &task.metadata.milestone {
                *milestone_counts.entry(milestone.as_str()).or_default() += 1;
            }
        }
    }

    let mut batch_ids = HashSet::new();
    for task in new_tasks {
        validate_new_task_shape(task)?;
        let (path, _) = render_task(root, task);
        if path.exists() && !skip_existing {
            return Err(TaskError::Message(format!(
                "task file already exists: {}",
                path.display()
            )));
        }
        if known_ids.contains(task.id.as_str()) && !skip_existing {
            return Err(TaskError::Message(format!(
                "task {} already exists",
                task.id
            )));
        }
        if !batch_ids.insert(task.id.as_str()) {
            return Err(TaskError::Message(format!(
                "task {} appears more than once in import",
                task.id
            )));
        }
        known_ids.insert(task.id.as_str());
        if task.task_type == TaskType::Milestone {
            let milestone = task.milestone.as_deref().ok_or_else(|| {
                TaskError::Message("Milestone tasks must include --milestone".to_string())
            })?;
            *milestone_counts.entry(milestone).or_default() += 1;
        }
    }

    for task in new_tasks {
        if let Some(parent) = &task.parent {
            if !is_valid_task_id(parent) {
                return Err(TaskError::Message(format!(
                    "invalid parent id {parent}; expected PREFIX-NUMBER"
                )));
            }
            if !known_ids.contains(parent.as_str()) {
                return Err(TaskError::Message(format!(
                    "parent {parent} does not reference an existing or imported task"
                )));
            }
        }
        for depends_on in &task.depends_on {
            if !is_valid_task_id(depends_on) {
                return Err(TaskError::Message(format!(
                    "invalid dependency id {depends_on}; expected PREFIX-NUMBER"
                )));
            }
            if !known_ids.contains(depends_on.as_str()) {
                return Err(TaskError::Message(format!(
                    "dependency {depends_on} does not reference an existing or imported task"
                )));
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
    validate_new_dependency_cycles(new_tasks)?;

    Ok(())
}

fn validate_new_task_shape(task: &NewTask) -> Result<(), TaskError> {
    if !is_valid_task_id(&task.id) {
        return Err(TaskError::Message(format!(
            "invalid id {}; expected PREFIX-NUMBER",
            task.id
        )));
    }
    if task.title.chars().count() < 5 {
        return Err(TaskError::Message(format!(
            "{} title must be at least 5 characters",
            task.id
        )));
    }
    let valid_domain = DOMAINS.contains(&task.domain.as_str()) || task.domain == MILESTONE_DOMAIN;
    if !valid_domain {
        return Err(TaskError::Message(format!(
            "domain {} is not a task domain",
            task.domain
        )));
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
    if prefix == "MILESTONE" && task.domain != MILESTONE_DOMAIN {
        return Err(TaskError::Message(
            "MILESTONE prefix is only allowed under .tasks/milestones".to_string(),
        ));
    }
    if !allowed_prefixes(&task.domain).contains(&prefix) {
        return Err(TaskError::Message(format!(
            "prefix {prefix} is not allowed for {} tasks",
            task.domain
        )));
    }
    let mut seen_dependencies = HashSet::new();
    for depends_on in &task.depends_on {
        if !seen_dependencies.insert(depends_on) {
            return Err(TaskError::Message(format!(
                "duplicate dependency id {depends_on}"
            )));
        }
        if depends_on == &task.id {
            return Err(TaskError::Message(
                "task cannot depend on itself".to_string(),
            ));
        }
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
    Ok(())
}

fn validate_new_dependency_cycles(new_tasks: &[&NewTask]) -> Result<(), TaskError> {
    let by_id: HashMap<&str, &NewTask> = new_tasks
        .iter()
        .map(|task| (task.id.as_str(), *task))
        .collect();
    let mut checked = HashSet::new();
    for task in new_tasks {
        let mut visiting = Vec::new();
        detect_new_cycle(task.id.as_str(), &by_id, &mut visiting, &mut checked)?;
    }
    Ok(())
}

fn detect_new_cycle<'a>(
    id: &'a str,
    by_id: &HashMap<&'a str, &'a NewTask>,
    visiting: &mut Vec<&'a str>,
    checked: &mut HashSet<&'a str>,
) -> Result<(), TaskError> {
    if checked.contains(id) {
        return Ok(());
    }
    if let Some(position) = visiting.iter().position(|current| *current == id) {
        let cycle = visiting[position..].join(" -> ");
        return Err(TaskError::Message(format!(
            "dependency cycle detected: {cycle} -> {id}"
        )));
    }
    let Some(task) = by_id.get(id) else {
        return Ok(());
    };
    visiting.push(id);
    for depends_on in &task.depends_on {
        if by_id.contains_key(depends_on.as_str()) {
            detect_new_cycle(depends_on, by_id, visiting, checked)?;
        }
    }
    visiting.pop();
    checked.insert(id);
    Ok(())
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
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn existing_ids(root: &Path) -> Result<HashSet<String>, TaskError> {
    Ok(require_valid_report(validate_repo(root)?)?
        .into_iter()
        .map(|task| task.metadata.id)
        .collect())
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

pub fn start_task(root: &Path, id: &str) -> Result<PathBuf, TaskError> {
    update_status(root, id, TaskStatus::InProgress, None)
}

pub fn block_task(root: &Path, id: &str, reason: &str) -> Result<PathBuf, TaskError> {
    update_status(
        root,
        id,
        TaskStatus::Blocked,
        Some(("Blocked Reason", reason)),
    )
}

pub fn defer_task(root: &Path, id: &str, reason: &str) -> Result<PathBuf, TaskError> {
    update_status(root, id, TaskStatus::Deferred, Some(("Rationale", reason)))
}

pub fn obsolete_task(root: &Path, id: &str, reason: &str) -> Result<PathBuf, TaskError> {
    update_status(root, id, TaskStatus::Obsolete, Some(("Rationale", reason)))
}

pub fn done_task(root: &Path, id: &str) -> Result<PathBuf, TaskError> {
    let path = task_path(root, id)?;
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
            "{id} cannot be marked Done while ## {heading} has unchecked items"
        )));
    }
    write_status(&path, &content, TaskStatus::Done, None)?;
    Ok(path)
}

fn update_status(
    root: &Path,
    id: &str,
    status: TaskStatus,
    section: Option<(&str, &str)>,
) -> Result<PathBuf, TaskError> {
    let path = task_path(root, id)?;
    let content = read(&path)?;
    write_status(&path, &content, status, section)?;
    Ok(path)
}

fn task_path(root: &Path, id: &str) -> Result<PathBuf, TaskError> {
    let tasks = require_valid_report(validate_repo(root)?)?;
    tasks
        .into_iter()
        .find(|task| task.metadata.id == id)
        .map(|task| task.path)
        .ok_or_else(|| TaskError::NotFound { id: id.to_string() })
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
