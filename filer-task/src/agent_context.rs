//! # Agent Context
//!
//! This module builds bounded task views for coding agents. It uses only task
//! declarations and named architecture rules so context stays authoritative.
//!
//! ```
//! use filer_task::model::TaskStatus;
//!
//! assert_eq!(TaskStatus::ToDo.to_string(), "To Do");
//! ```

use std::{collections::HashSet, fs, path::Path};

use serde::Serialize;

use crate::{
    error::TaskError,
    markdown::{ChecklistItem, MarkdownSection, checklist_items, level_two_sections, section},
    model::{Priority, Task, TaskMetadata, TaskStatus, TaskType},
    project::TaskProject,
};

const AGENT_SCHEMA_VERSION: u32 = 1;
const INVARIANTS_PATH: &str = "docs/architecture/invariants.md";

#[derive(Debug, Clone, Default)]
pub struct ReadyFilter {
    pub domain: Option<String>,
    pub milestone: Option<String>,
    pub priority: Option<Priority>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskView {
    pub path: String,
    pub domain: String,
    #[serde(flatten)]
    pub metadata: TaskMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDetail {
    pub task: TaskView,
    pub sections: Vec<MarkdownSection>,
    pub criteria: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShowView {
    pub schema_version: u32,
    pub detail: TaskDetail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadyView {
    pub schema_version: u32,
    pub tasks: Vec<TaskView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadinessView {
    pub ready: bool,
    pub blockers: Vec<ReadinessBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadinessBlocker {
    pub kind: ReadinessBlockerKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessBlockerKind {
    TaskStatus,
    Milestone,
    HasChildren,
    Dependency,
    AncestorStatus,
    AncestorCycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleReference {
    pub id: String,
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedTaskView {
    pub task: TaskView,
    pub readiness: ReadinessView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextView {
    pub schema_version: u32,
    pub detail: TaskDetail,
    pub readiness: ReadinessView,
    pub parent: Option<RelatedTaskView>,
    pub children: Vec<RelatedTaskView>,
    pub dependencies: Vec<RelatedTaskView>,
    pub dependents: Vec<RelatedTaskView>,
    pub milestone: Option<RelatedTaskView>,
    pub rules: Vec<RuleReference>,
    pub whitepaper: Option<String>,
}

pub fn build_show(project: &TaskProject, tasks: &[Task], id: &str) -> Result<ShowView, TaskError> {
    let task = find_task(tasks, id)?;
    Ok(ShowView {
        schema_version: AGENT_SCHEMA_VERSION,
        detail: task_detail(project.root(), task)?,
    })
}

pub fn build_ready(project: &TaskProject, tasks: &[Task], filter: &ReadyFilter) -> ReadyView {
    let mut ready: Vec<&Task> = tasks
        .iter()
        .filter(|task| matches_filter(task, filter))
        .filter(|task| readiness(tasks, task).ready)
        .collect();
    ready.sort_by(|left, right| {
        priority_order(left.metadata.priority)
            .cmp(&priority_order(right.metadata.priority))
            .then_with(|| left.metadata.id.cmp(&right.metadata.id))
    });
    if let Some(limit) = filter.limit {
        ready.truncate(limit);
    }

    ReadyView {
        schema_version: AGENT_SCHEMA_VERSION,
        tasks: ready
            .into_iter()
            .map(|task| task_view(project.root(), task))
            .collect(),
    }
}

pub fn build_context(
    project: &TaskProject,
    tasks: &[Task],
    id: &str,
) -> Result<ContextView, TaskError> {
    let root = project.root();
    let task = find_task(tasks, id)?;
    let children = tasks
        .iter()
        .filter(|candidate| candidate.metadata.parent.as_deref() == Some(id))
        .map(|candidate| related_task_view(root, tasks, candidate))
        .collect();
    let dependencies = task
        .metadata
        .depends_on
        .iter()
        .filter_map(|dependency| {
            tasks
                .iter()
                .find(|candidate| candidate.metadata.id == *dependency)
        })
        .map(|candidate| related_task_view(root, tasks, candidate))
        .collect();
    let dependents = tasks
        .iter()
        .filter(|candidate| {
            candidate
                .metadata
                .depends_on
                .iter()
                .any(|dependency| dependency == id)
        })
        .map(|candidate| related_task_view(root, tasks, candidate))
        .collect();
    let parent = task
        .metadata
        .parent
        .as_ref()
        .and_then(|parent| {
            tasks
                .iter()
                .find(|candidate| candidate.metadata.id == *parent)
        })
        .map(|candidate| related_task_view(root, tasks, candidate));
    let milestone = task.metadata.milestone.as_ref().and_then(|milestone| {
        tasks.iter().find(|candidate| {
            candidate.metadata.task_type == TaskType::Milestone
                && candidate.metadata.milestone.as_ref() == Some(milestone)
        })
    });

    Ok(ContextView {
        schema_version: AGENT_SCHEMA_VERSION,
        detail: task_detail(root, task)?,
        readiness: readiness(tasks, task),
        parent,
        children,
        dependencies,
        dependents,
        milestone: milestone.map(|candidate| related_task_view(root, tasks, candidate)),
        rules: rule_references(root, &task.metadata.rules)?,
        whitepaper: task.metadata.whitepaper.as_deref().map(normalize_path),
    })
}

fn task_detail(root: &Path, task: &Task) -> Result<TaskDetail, TaskError> {
    let content = fs::read_to_string(&task.path).map_err(|source| TaskError::Io {
        path: task.path.clone(),
        source,
    })?;
    let criteria_heading = task.metadata.task_type.criteria_heading();
    // The criteria heading is exposed structurally through `criteria`, so drop
    // it from `sections` to avoid emitting the same checklist text twice.
    let sections = level_two_sections(&content)
        .into_iter()
        .filter(|section| section.heading != criteria_heading)
        .collect();
    Ok(TaskDetail {
        task: task_view(root, task),
        sections,
        criteria: checklist_items(&content, criteria_heading),
    })
}

fn task_view(root: &Path, task: &Task) -> TaskView {
    let path = task.path.strip_prefix(root).unwrap_or(&task.path);
    TaskView {
        path: normalize_path(path),
        domain: task.domain.clone(),
        metadata: task.metadata.clone(),
    }
}

fn related_task_view(root: &Path, tasks: &[Task], task: &Task) -> RelatedTaskView {
    RelatedTaskView {
        task: task_view(root, task),
        readiness: readiness(tasks, task),
    }
}

fn readiness(tasks: &[Task], task: &Task) -> ReadinessView {
    let mut blockers = Vec::new();
    if task.metadata.status != TaskStatus::ToDo {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::TaskStatus,
            task_id: Some(task.metadata.id.clone()),
            status: Some(task.metadata.status),
        });
    }
    if task.metadata.task_type == TaskType::Milestone {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::Milestone,
            task_id: Some(task.metadata.id.clone()),
            status: None,
        });
    }
    if tasks
        .iter()
        .any(|candidate| candidate.metadata.parent.as_ref() == Some(&task.metadata.id))
    {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::HasChildren,
            task_id: Some(task.metadata.id.clone()),
            status: None,
        });
    }
    for dependency in &task.metadata.depends_on {
        if let Some(candidate) = tasks
            .iter()
            .find(|candidate| candidate.metadata.id == *dependency)
        {
            if candidate.metadata.status != TaskStatus::Done {
                blockers.push(ReadinessBlocker {
                    kind: ReadinessBlockerKind::Dependency,
                    task_id: Some(candidate.metadata.id.clone()),
                    status: Some(candidate.metadata.status),
                });
            }
        }
    }
    append_ancestor_blockers(tasks, task, &mut blockers);
    ReadinessView {
        ready: blockers.is_empty(),
        blockers,
    }
}

fn append_ancestor_blockers(tasks: &[Task], task: &Task, blockers: &mut Vec<ReadinessBlocker>) {
    let mut parent = task.metadata.parent.as_deref();
    let mut visited = HashSet::new();
    while let Some(parent_id) = parent {
        if !visited.insert(parent_id) {
            blockers.push(ReadinessBlocker {
                kind: ReadinessBlockerKind::AncestorCycle,
                task_id: Some(parent_id.to_string()),
                status: None,
            });
            return;
        }
        let Some(ancestor) = tasks
            .iter()
            .find(|candidate| candidate.metadata.id == parent_id)
        else {
            return;
        };
        if !matches!(
            ancestor.metadata.status,
            TaskStatus::ToDo | TaskStatus::InProgress
        ) {
            blockers.push(ReadinessBlocker {
                kind: ReadinessBlockerKind::AncestorStatus,
                task_id: Some(ancestor.metadata.id.clone()),
                status: Some(ancestor.metadata.status),
            });
        }
        parent = ancestor.metadata.parent.as_deref();
    }
}

fn rule_references(root: &Path, rule_ids: &[String]) -> Result<Vec<RuleReference>, TaskError> {
    if rule_ids.is_empty() {
        return Ok(Vec::new());
    }
    let path = root.join(INVARIANTS_PATH);
    let content = fs::read_to_string(&path).map_err(|source| TaskError::Io {
        path: path.clone(),
        source,
    })?;
    rule_ids
        .iter()
        .map(|id| {
            let text = section(&content, id).ok_or_else(|| {
                TaskError::Message(format!(
                    "rule {id} does not have a section in {INVARIANTS_PATH}"
                ))
            })?;
            Ok(RuleReference {
                id: id.clone(),
                path: INVARIANTS_PATH.to_string(),
                text,
            })
        })
        .collect()
}

fn matches_filter(task: &Task, filter: &ReadyFilter) -> bool {
    filter
        .domain
        .as_ref()
        .is_none_or(|domain| &task.domain == domain)
        && filter
            .milestone
            .as_ref()
            .is_none_or(|milestone| task.metadata.milestone.as_ref() == Some(milestone))
        && filter
            .priority
            .is_none_or(|priority| task.metadata.priority == priority)
        && filter
            .tag
            .as_ref()
            .is_none_or(|tag| task.metadata.tags.iter().any(|value| value == tag))
}

fn find_task<'a>(tasks: &'a [Task], id: &str) -> Result<&'a Task, TaskError> {
    tasks
        .iter()
        .find(|task| task.metadata.id == id)
        .ok_or_else(|| TaskError::NotFound { id: id.to_string() })
}

fn priority_order(priority: Priority) -> u8 {
    match priority {
        Priority::High => 0,
        Priority::Medium => 1,
        Priority::Low => 2,
    }
}

fn normalize_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

impl ReadinessBlockerKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::TaskStatus => "task_status",
            Self::Milestone => "milestone",
            Self::HasChildren => "has_children",
            Self::Dependency => "dependency",
            Self::AncestorStatus => "ancestor_status",
            Self::AncestorCycle => "ancestor_cycle",
        }
    }
}
