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

use std::{fs, path::Path};

use serde::{Serialize, Serializer};

use crate::{
    error::TaskError,
    graph::TaskGraph,
    identity::TaskIdentity,
    markdown::{
        HashedChecklistItem, MarkdownSection, hashed_checklist_items, level_two_sections, section,
    },
    model::{Priority, Task, TaskMetadata, TaskStatus},
    project::TaskProject,
    taxonomy::{criteria_heading, is_milestone_type, validate_tag},
    validate::ValidationWarning,
};

const AGENT_SCHEMA_VERSION: u32 = 2;
const INVARIANTS_PATH: &str = "docs/architecture/invariants.md";

#[derive(Debug, Clone, Default)]
pub struct ReadyFilter {
    pub domain: Option<String>,
    pub milestone: Option<String>,
    pub priority: Option<Priority>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    pub path: String,
    pub domain: String,
    pub metadata: TaskMetadata,
}

impl TaskView {
    pub fn identity(&self) -> TaskIdentity {
        TaskIdentity {
            domain: self.domain.clone(),
            id: self.metadata.id.clone(),
        }
    }

    pub fn qualified_id(&self) -> String {
        self.identity().to_string()
    }
}

impl Serialize for TaskView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TaskViewOutput<'a> {
            path: &'a str,
            domain: &'a str,
            qualified_id: String,
            #[serde(flatten)]
            metadata: &'a TaskMetadata,
        }

        TaskViewOutput {
            path: &self.path,
            domain: &self.domain,
            qualified_id: self.qualified_id(),
            metadata: &self.metadata,
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDetail {
    pub task: TaskView,
    pub sections: Vec<MarkdownSection>,
    pub criteria_heading: String,
    pub criteria: Vec<HashedChecklistItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShowView {
    pub schema_version: u32,
    pub warnings: Vec<ValidationWarning>,
    pub detail: TaskDetail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadyView {
    pub schema_version: u32,
    pub warnings: Vec<ValidationWarning>,
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
    pub warnings: Vec<ValidationWarning>,
    pub detail: TaskDetail,
    pub readiness: ReadinessView,
    pub parent: Option<RelatedTaskView>,
    /// Root-first breadcrumb of every ancestor. Plain views, not related views:
    /// a breadcrumb needs identity and status, and per-ancestor readiness would
    /// invite reading a parent's blockers as the target task's own.
    pub ancestors: Vec<TaskView>,
    pub children: Vec<RelatedTaskView>,
    pub dependencies: Vec<RelatedTaskView>,
    pub dependents: Vec<RelatedTaskView>,
    pub milestone: Option<RelatedTaskView>,
    pub rules: Vec<RuleReference>,
    pub whitepaper: Option<String>,
}

pub fn build_show(
    project: &TaskProject,
    tasks: &[Task],
    identity: &TaskIdentity,
    warnings: &[ValidationWarning],
) -> Result<ShowView, TaskError> {
    let graph = TaskGraph::new(project, tasks)?;
    let task = require_task(project, &graph, identity)?;
    Ok(ShowView {
        schema_version: AGENT_SCHEMA_VERSION,
        warnings: warnings.to_vec(),
        detail: task_detail(project, &graph, task)?,
    })
}

pub fn build_ready(
    project: &TaskProject,
    tasks: &[Task],
    filter: &ReadyFilter,
    warnings: &[ValidationWarning],
) -> Result<ReadyView, TaskError> {
    if let Some(tag) = &filter.tag {
        validate_tag(project, tag, "tag", None, None)?;
    }
    let graph = TaskGraph::new(project, tasks)?;
    let mut ready: Vec<&Task> = tasks
        .iter()
        .filter(|task| matches_filter(task, filter))
        .filter(|task| readiness(project, &graph, task).ready)
        .collect();
    ready.sort_by(|left, right| {
        priority_order(left.metadata.priority)
            .cmp(&priority_order(right.metadata.priority))
            .then_with(|| left.identity().cmp(&right.identity()))
    });
    if let Some(limit) = filter.limit {
        ready.truncate(limit);
    }

    Ok(ReadyView {
        schema_version: AGENT_SCHEMA_VERSION,
        warnings: warnings.to_vec(),
        tasks: ready
            .into_iter()
            .map(|task| task_view(project.root(), &graph, task))
            .collect(),
    })
}

pub fn build_context(
    project: &TaskProject,
    tasks: &[Task],
    identity: &TaskIdentity,
    warnings: &[ValidationWarning],
) -> Result<ContextView, TaskError> {
    let root = project.root();
    let graph = TaskGraph::new(project, tasks)?;
    let task = require_task(project, &graph, identity)?;
    let children = graph
        .children(identity)
        .into_iter()
        .map(|candidate| related_task_view(project, &graph, candidate))
        .collect();
    let dependencies = graph
        .dependencies(identity)
        .into_iter()
        .map(|candidate| related_task_view(project, &graph, candidate))
        .collect();
    let dependents = graph
        .dependents(identity)
        .into_iter()
        .map(|candidate| related_task_view(project, &graph, candidate))
        .collect();
    let parent = graph
        .parent(identity)
        .map(|candidate| related_task_view(project, &graph, candidate));
    let mut ancestors = graph
        .ancestor_identities(identity)
        .ancestors
        .iter()
        .filter_map(|ancestor| graph.task(ancestor))
        .map(|ancestor| task_view(root, &graph, ancestor))
        .collect::<Vec<_>>();
    ancestors.reverse();
    let milestone = task.metadata.milestone.as_ref().and_then(|milestone| {
        tasks.iter().find(|candidate| {
            is_milestone_type(project, &candidate.metadata.task_type)
                && candidate.metadata.milestone.as_ref() == Some(milestone)
        })
    });

    Ok(ContextView {
        schema_version: AGENT_SCHEMA_VERSION,
        warnings: warnings.to_vec(),
        detail: task_detail(project, &graph, task)?,
        readiness: readiness(project, &graph, task),
        parent,
        ancestors,
        children,
        dependencies,
        dependents,
        milestone: milestone.map(|candidate| related_task_view(project, &graph, candidate)),
        rules: rule_references(root, &task.metadata.rules)?,
        whitepaper: task.metadata.whitepaper.as_deref().map(normalize_path),
    })
}

fn task_detail(
    project: &TaskProject,
    graph: &TaskGraph<'_>,
    task: &Task,
) -> Result<TaskDetail, TaskError> {
    let content = fs::read_to_string(&task.path).map_err(|source| TaskError::Io {
        path: task.path.clone(),
        source,
    })?;
    let criteria_heading = criteria_heading(
        project,
        &task.metadata.task_type,
        Some(&task.domain),
        Some(&task.qualified_id()),
    )?;
    // The criteria heading is exposed structurally through `criteria`, so drop
    // it from `sections` to avoid emitting the same checklist text twice.
    let sections = level_two_sections(&content)
        .into_iter()
        .filter(|section| section.heading != criteria_heading)
        .collect();
    Ok(TaskDetail {
        task: task_view(project.root(), graph, task),
        sections,
        criteria_heading: criteria_heading.to_string(),
        criteria: hashed_checklist_items(&content, criteria_heading),
    })
}

pub(crate) fn task_view(root: &Path, graph: &TaskGraph<'_>, task: &Task) -> TaskView {
    let path = task.path.strip_prefix(root).unwrap_or(&task.path);
    let identity = task.identity();
    let mut metadata = task.metadata.clone();
    metadata.parent = graph.parent_identity(&identity).map(ToString::to_string);
    metadata.depends_on = graph
        .dependency_identities(&identity)
        .iter()
        .map(ToString::to_string)
        .collect();
    TaskView {
        path: normalize_path(path),
        domain: task.domain.clone(),
        metadata,
    }
}

fn related_task_view(project: &TaskProject, graph: &TaskGraph<'_>, task: &Task) -> RelatedTaskView {
    RelatedTaskView {
        task: task_view(project.root(), graph, task),
        readiness: readiness(project, graph, task),
    }
}

fn readiness(project: &TaskProject, graph: &TaskGraph<'_>, task: &Task) -> ReadinessView {
    let mut blockers = Vec::new();
    let identity = task.identity();
    if task.metadata.status != TaskStatus::ToDo {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::TaskStatus,
            task_id: Some(identity.to_string()),
            status: Some(task.metadata.status),
        });
    }
    if is_milestone_type(project, &task.metadata.task_type) {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::Milestone,
            task_id: Some(identity.to_string()),
            status: None,
        });
    }
    if !graph.children(&identity).is_empty() {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::HasChildren,
            task_id: Some(identity.to_string()),
            status: None,
        });
    }
    for candidate in graph.dependencies(&identity) {
        if candidate.metadata.status != TaskStatus::Done {
            blockers.push(ReadinessBlocker {
                kind: ReadinessBlockerKind::Dependency,
                task_id: Some(candidate.qualified_id()),
                status: Some(candidate.metadata.status),
            });
        }
    }
    append_ancestor_blockers(graph, task, &mut blockers);
    ReadinessView {
        ready: blockers.is_empty(),
        blockers,
    }
}

fn append_ancestor_blockers(
    graph: &TaskGraph<'_>,
    task: &Task,
    blockers: &mut Vec<ReadinessBlocker>,
) {
    let chain = graph.ancestor_identities(&task.identity());
    for identity in chain.ancestors {
        let Some(ancestor) = graph.task(&identity) else {
            continue;
        };
        if !matches!(
            ancestor.metadata.status,
            TaskStatus::ToDo | TaskStatus::InProgress
        ) {
            blockers.push(ReadinessBlocker {
                kind: ReadinessBlockerKind::AncestorStatus,
                task_id: Some(identity.to_string()),
                status: Some(ancestor.metadata.status),
            });
        }
    }
    if let Some(identity) = chain.cycle {
        blockers.push(ReadinessBlocker {
            kind: ReadinessBlockerKind::AncestorCycle,
            task_id: Some(identity.to_string()),
            status: None,
        });
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

fn require_task<'a>(
    project: &TaskProject,
    graph: &TaskGraph<'a>,
    identity: &TaskIdentity,
) -> Result<&'a Task, TaskError> {
    graph.task(identity).ok_or_else(|| TaskError::TaskNotFound {
        reference: identity.to_string(),
        source_domain: None,
        root: project.root().to_path_buf(),
    })
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
