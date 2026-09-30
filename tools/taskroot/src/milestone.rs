//! # Milestone Aggregation
//!
//! This module builds milestone progress from the project policy and task
//! bindings. Keeping the grouping here lets every caller share one definition
//! of milestone scope without depending on domain or type compatibility names.
//!
//! ```
//! use taskroot::{milestone::tasks_for_milestone, model::Task};
//!
//! let tasks: Vec<Task> = Vec::new();
//! assert_eq!(tasks_for_milestone(&tasks, "1.0.0").count(), 0);
//! ```

use std::{collections::BTreeMap, fs};

use serde::Serialize;

use crate::{
    agent_context::{TaskView, task_view},
    error::TaskError,
    graph::TaskGraph,
    markdown::{ChecklistItem, checklist_items},
    model::{Task, TaskStatus},
    project::TaskProject,
    taxonomy::{criteria_heading, is_milestone_type},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MilestoneAggregation {
    pub milestone: TaskView,
    pub criteria_heading: String,
    pub criteria: Vec<ChecklistItem>,
    pub done: usize,
    pub total: usize,
    pub tasks_by_status: BTreeMap<String, Vec<TaskView>>,
}

pub fn build_milestone_aggregations(
    project: &TaskProject,
    tasks: &[Task],
) -> Result<Vec<MilestoneAggregation>, TaskError> {
    let graph = TaskGraph::new(project, tasks)?;
    let mut declarations: Vec<&Task> = tasks
        .iter()
        .filter(|task| is_milestone_type(project, &task.metadata.task_type))
        .collect();
    declarations.sort_by_key(|task| task.identity());

    declarations
        .into_iter()
        .map(|declaration| {
            let milestone_value = declaration.metadata.milestone.as_deref().ok_or_else(|| {
                TaskError::Message(format!(
                    "milestone-role task {} has no milestone value",
                    declaration.qualified_id()
                ))
            })?;
            let content =
                fs::read_to_string(&declaration.path).map_err(|source| TaskError::Io {
                    path: declaration.path.clone(),
                    source,
                })?;
            let heading = criteria_heading(
                project,
                &declaration.metadata.task_type,
                Some(&declaration.domain),
                Some(&declaration.qualified_id()),
            )?;
            let mut scoped: Vec<&Task> = tasks_for_milestone(tasks, milestone_value).collect();
            scoped.sort_by_key(|task| task.identity());
            let done = scoped
                .iter()
                .filter(|task| task.metadata.status == TaskStatus::Done)
                .count();
            let total = scoped.len();
            let mut tasks_by_status: BTreeMap<String, Vec<TaskView>> = BTreeMap::new();
            for task in scoped {
                tasks_by_status
                    .entry(task.metadata.status.to_string())
                    .or_default()
                    .push(task_view(project.root(), &graph, task));
            }

            Ok(MilestoneAggregation {
                milestone: task_view(project.root(), &graph, declaration),
                criteria_heading: heading.to_string(),
                criteria: checklist_items(&content, heading),
                done,
                total,
                tasks_by_status,
            })
        })
        .collect()
}

pub fn tasks_for_milestone<'a>(
    tasks: &'a [Task],
    milestone: &'a str,
) -> impl Iterator<Item = &'a Task> + 'a {
    tasks
        .iter()
        .filter(move |task| task.metadata.milestone.as_deref() == Some(milestone))
}
