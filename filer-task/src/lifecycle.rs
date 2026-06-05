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
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    error::TaskError,
    frontmatter::parse_metadata,
    markdown::{has_unchecked_checklist_item, replace_or_append_section},
    model::{Priority, TaskStatus, TaskType},
    repo::{DOMAINS, MILESTONE_DOMAIN, TASK_DIR},
    validate::{require_valid_report, validate_repo},
};

pub struct NewTask {
    pub domain: String,
    pub id: String,
    pub title: String,
    pub priority: Priority,
    pub task_type: TaskType,
    pub milestone: Option<String>,
}

pub fn add_task(root: &Path, task: NewTask) -> Result<PathBuf, TaskError> {
    validate_new_task(root, &task)?;
    let relative = format!("{}-{}.md", task.id, slug(&task.title));
    let path = root.join(TASK_DIR).join(&task.domain).join(relative);
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

    let criteria_heading = if matches!(task.task_type, TaskType::Milestone | TaskType::Epic) {
        "Exit Criteria"
    } else {
        "Acceptance Criteria"
    };
    let milestone = task
        .milestone
        .as_ref()
        .map(|value| format!("milestone: \"{value}\"\n"))
        .unwrap_or_default();
    let content = format!(
        "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: {priority}\ntype: {task_type}\n{milestone}last_updated: {date}\n---\n\n## Summary\n\nDescribe why this work exists.\n\n## {criteria_heading}\n\n- [ ] Define completion criteria.\n",
        id = task.id,
        title = task.title,
        priority = task.priority,
        task_type = task.task_type,
        milestone = milestone,
        date = today()
    );

    fs::write(&path, content).map_err(|source| TaskError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn validate_new_task(root: &Path, task: &NewTask) -> Result<(), TaskError> {
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

    let tasks = require_valid_report(validate_repo(root)?)?;
    if tasks.iter().any(|existing| existing.metadata.id == task.id) {
        return Err(TaskError::Message(format!(
            "task {} already exists",
            task.id
        )));
    }

    if task.task_type == TaskType::Milestone {
        if task.milestone.is_none() {
            return Err(TaskError::Message(
                "Milestone tasks must include --milestone".to_string(),
            ));
        }
    } else if let Some(milestone) = &task.milestone {
        let count = tasks
            .iter()
            .filter(|existing| {
                existing.metadata.task_type == TaskType::Milestone
                    && existing.metadata.milestone.as_ref() == Some(milestone)
            })
            .count();
        if count != 1 {
            return Err(TaskError::Message(format!(
                "milestone {milestone} must reference exactly one milestone task"
            )));
        }
    }

    Ok(())
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
        .ok_or_else(|| TaskError::Message(format!("task {id} does not exist")))
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
