//! # Human Output
//!
//! This module renders task data before it reaches stdout. Keeping width and
//! truncation rules here makes command output deterministic and testable.
//!
//! ```
//! let columns = ["TASK", "TITLE"];
//! assert_eq!(columns.join("  "), "TASK  TITLE");
//! ```

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::{
    agent_context::{
        ContextView, ReadinessView, ReadyView, RelatedTaskView, ShowView, TaskDetail, TaskView,
    },
    markdown::ChecklistItem,
    model::Task,
    validate::ValidationWarning,
};

#[derive(Serialize)]
pub(crate) struct ValidationOutput<'a> {
    pub task_count: usize,
    pub warnings: &'a [ValidationWarning],
}

pub(crate) struct TaskActionOutput<'a> {
    pub action: TaskAction,
    pub task_id: &'a str,
    pub root: &'a Path,
    pub path: &'a Path,
}

pub(crate) enum TaskAction {
    Created,
    Started,
    Completed,
    Blocked,
    Deferred,
    Obsolete,
}

pub(crate) struct ImportOutput<'a> {
    pub dry_run: bool,
    pub root: &'a Path,
    pub paths: &'a [PathBuf],
}

pub(crate) struct MilestoneOutput<'a> {
    pub milestone: &'a str,
    pub title: &'a str,
    pub summary: &'a SummaryOutput,
    pub criteria_heading: &'a str,
    pub exit_criteria: Option<&'a [ChecklistItem]>,
    pub open_tasks: &'a [Task],
}

#[derive(Debug, Serialize)]
pub(crate) struct SummaryOutput {
    pub status: BTreeMap<String, usize>,
    pub domain: BTreeMap<String, usize>,
    pub priority: BTreeMap<String, usize>,
}

const COLUMN_GAP: &str = "  ";
const MAX_TITLE_CHARS: usize = 48;
const ELLIPSIS_CHARS: usize = 3;
const TASK_HEADERS: [&str; 8] = [
    "TASK",
    "STATUS",
    "TYPE",
    "PRIORITY",
    "RISK",
    "DOMAIN",
    "MILESTONE",
    "TITLE",
];

pub(crate) fn render_tasks(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "No tasks.".to_string();
    }

    let rows: Vec<[String; 8]> = tasks
        .iter()
        .map(|task| task_row(&task.qualified_id(), &task.domain, &task.metadata))
        .collect();
    render_task_rows(&rows)
}

fn render_task_rows(rows: &[[String; 8]]) -> String {
    let widths = column_widths(rows);
    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(render_row(&TASK_HEADERS, &widths));
    let underline = widths.map(|width| "-".repeat(width));
    lines.push(render_row(&underline, &widths));
    lines.extend(rows.iter().map(|row| render_row(row, &widths)));
    lines.join("\n")
}

pub(crate) fn render_summary(sections: &[(&str, &BTreeMap<String, usize>)]) -> String {
    let label_width = sections
        .iter()
        .flat_map(|(_, counts)| counts.keys())
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
    let count_width = sections
        .iter()
        .flat_map(|(_, counts)| counts.values())
        .map(|count| count.to_string().len())
        .max()
        .unwrap_or(1);
    let mut rendered = Vec::with_capacity(sections.len());

    for (heading, counts) in sections {
        let mut lines = vec![(*heading).to_string()];
        lines.extend(
            counts
                .iter()
                .map(|(label, count)| format!("{label:<label_width$}: {count:>count_width$}")),
        );
        rendered.push(lines.join("\n"));
    }

    rendered.join("\n\n")
}

pub(crate) fn render_summary_output(summary: &SummaryOutput) -> String {
    render_summary(&[
        ("Status", &summary.status),
        ("Domain", &summary.domain),
        ("Priority", &summary.priority),
    ])
}

pub(crate) fn render_milestone(output: &MilestoneOutput<'_>) -> String {
    let mut sections = vec![
        format!("Milestone {}: {}", output.milestone, output.title),
        render_summary_output(output.summary),
    ];
    if let Some(criteria) = output.exit_criteria {
        let rendered = criteria
            .iter()
            .map(|item| {
                let marker = if item.checked { "x" } else { " " };
                format!("- [{marker}] {}", item.text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("{}\n{rendered}", output.criteria_heading));
    }
    sections.push(format!("Open Tasks\n{}", render_tasks(output.open_tasks)));
    sections.join("\n\n")
}

pub(crate) fn render_show(view: &ShowView) -> String {
    append_warnings(render_detail(&view.detail), &view.warnings)
}

pub(crate) fn render_ready(view: &ReadyView) -> String {
    let output = if view.tasks.is_empty() {
        "No ready tasks.".to_string()
    } else {
        render_task_views(&view.tasks)
    };
    append_warnings(output, &view.warnings)
}

pub(crate) fn render_context(view: &ContextView) -> String {
    let mut sections = vec![render_detail(&view.detail)];
    sections.push(render_readiness(&view.readiness));
    sections.push(render_relation("Parent", view.parent.as_ref().into_iter()));
    sections.push(render_relation("Children", view.children.iter()));
    sections.push(render_relation("Dependencies", view.dependencies.iter()));
    sections.push(render_relation("Dependents", view.dependents.iter()));
    sections.push(render_relation(
        "Milestone",
        view.milestone.as_ref().into_iter(),
    ));

    if !view.rules.is_empty() {
        let rules = view
            .rules
            .iter()
            .map(|rule| format!("{} ({})\n{}", rule.id, rule.path, rule.text))
            .collect::<Vec<_>>()
            .join("\n\n");
        sections.push(format!("Rules\n{rules}"));
    }
    if let Some(whitepaper) = &view.whitepaper {
        sections.push(format!("Whitepaper\n{whitepaper}"));
    }
    append_warnings(sections.join("\n\n"), &view.warnings)
}

pub(crate) fn render_validation(output: &ValidationOutput<'_>) -> String {
    append_warnings(
        format!(
            "Validation\nStatus: Passed\nTasks: {}\nWarnings: {}",
            output.task_count,
            output.warnings.len()
        ),
        output.warnings,
    )
}

pub(crate) fn render_task_action(output: &TaskActionOutput<'_>) -> String {
    format!(
        "{}\nTask: {}\nPath: {}",
        output.action.heading(),
        output.task_id,
        normalize_path(output.root, output.path)
    )
}

pub(crate) fn render_import(output: &ImportOutput<'_>) -> String {
    let mode = if output.dry_run { "Dry Run" } else { "Write" };
    let mut rendered = format!("Import\nMode: {mode}\nTasks: {}", output.paths.len());
    if !output.paths.is_empty() {
        rendered.push_str("\n\nPaths\n");
        rendered.push_str(
            &output
                .paths
                .iter()
                .map(|path| normalize_path(output.root, path))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    rendered
}

impl TaskAction {
    fn heading(&self) -> &'static str {
        match self {
            Self::Created => "Task Created",
            Self::Started => "Task Started",
            Self::Completed => "Task Completed",
            Self::Blocked => "Task Blocked",
            Self::Deferred => "Task Deferred",
            Self::Obsolete => "Task Obsolete",
        }
    }
}

fn normalize_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn render_detail(detail: &TaskDetail) -> String {
    let task = &detail.task;
    let metadata = &task.metadata;
    let mut output = format!(
        "{}: {}\nStatus: {}\nPriority: {}\nType: {}\nDomain: {}\nPath: {}\nParent: {}\nMilestone: {}\nDependencies: {}\nRules: {}\nRisk: {}\nImpact: {}\nTags: {}\nWhitepaper: {}\nLast Updated: {}",
        task.qualified_id(),
        metadata.title,
        metadata.status,
        metadata.priority,
        metadata.task_type,
        task.domain,
        task.path,
        metadata.parent.as_deref().unwrap_or("-"),
        metadata.milestone.as_deref().unwrap_or("-"),
        render_values(&metadata.depends_on),
        render_values(&metadata.rules),
        metadata
            .risk
            .map(|risk| risk.to_string())
            .unwrap_or_else(|| "-".to_string()),
        metadata.impact.as_deref().unwrap_or("-"),
        render_values(&metadata.tags),
        metadata.whitepaper.as_deref().unwrap_or("-"),
        metadata.last_updated.as_deref().unwrap_or("-"),
    );
    for section in &detail.sections {
        output.push_str(&format!("\n\n{}\n{}", section.heading, section.content));
    }
    if !detail.criteria.is_empty() {
        let items = detail
            .criteria
            .iter()
            .map(|item| {
                let mark = if item.checked { 'x' } else { ' ' };
                format!("- [{mark}] {}", item.text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        output.push_str(&format!("\n\n{}\n{items}", detail.criteria_heading));
    }
    output
}

fn render_readiness(readiness: &ReadinessView) -> String {
    if readiness.ready {
        return "Readiness\nReady".to_string();
    }
    let blockers = readiness
        .blockers
        .iter()
        .map(|blocker| {
            let task = blocker.task_id.as_deref().unwrap_or("-");
            let status = blocker
                .status
                .map(|status| format!(" ({status})"))
                .unwrap_or_default();
            format!("- {}: {task}{status}", blocker.kind.label())
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Readiness\nBlocked\n{blockers}")
}

fn render_values(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn render_relation<'a>(heading: &str, tasks: impl Iterator<Item = &'a RelatedTaskView>) -> String {
    let tasks: Vec<&RelatedTaskView> = tasks.collect();
    if tasks.is_empty() {
        format!("{heading}\nNone")
    } else {
        let rendered = tasks
            .iter()
            .map(|related| {
                let state = if related.readiness.ready {
                    "ready"
                } else {
                    "not ready"
                };
                format!(
                    "{}  {}  {}  {}  {state}",
                    related.task.qualified_id(),
                    related.task.metadata.status,
                    related.task.metadata.priority,
                    related.task.metadata.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{heading}\n{rendered}")
    }
}

fn render_task_views<T: std::borrow::Borrow<TaskView>>(tasks: &[T]) -> String {
    let rows: Vec<[String; 8]> = tasks
        .iter()
        .map(|task| {
            let task = task.borrow();
            task_row(&task.qualified_id(), &task.domain, &task.metadata)
        })
        .collect();
    render_task_rows(&rows)
}

fn task_row(
    qualified_id: &str,
    domain: &str,
    metadata: &crate::model::TaskMetadata,
) -> [String; 8] {
    [
        qualified_id.to_string(),
        metadata.status.to_string(),
        metadata.task_type.to_string(),
        metadata.priority.to_string(),
        metadata
            .risk
            .map(|risk| risk.to_string())
            .unwrap_or_else(|| "-".to_string()),
        domain.to_string(),
        metadata
            .milestone
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        truncate_title(&metadata.title),
    ]
}

fn append_warnings(mut output: String, warnings: &[ValidationWarning]) -> String {
    if warnings.is_empty() {
        return output;
    }
    output.push_str("\n\nWarnings\n");
    output.push_str(
        &warnings
            .iter()
            .map(|warning| {
                let path = warning
                    .path
                    .as_ref()
                    .map(|path| format!(" {path}"))
                    .unwrap_or_default();
                format!("- [{}]{path}: {}", warning.code, warning.message)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    output
}

fn truncate_title(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title.to_string();
    }

    let prefix: String = title
        .chars()
        .take(MAX_TITLE_CHARS - ELLIPSIS_CHARS)
        .collect();
    format!("{prefix}...")
}

fn column_widths(rows: &[[String; 8]]) -> [usize; 8] {
    std::array::from_fn(|index| {
        rows.iter()
            .map(|row| row[index].chars().count())
            .chain(std::iter::once(TASK_HEADERS[index].len()))
            .max()
            .unwrap_or(0)
    })
}

fn render_row<T: AsRef<str>>(values: &[T; 8], widths: &[usize; 8]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let value = value.as_ref();
            if index + 1 == values.len() {
                value.to_string()
            } else {
                format!("{value:<width$}", width = widths[index])
            }
        })
        .collect::<Vec<_>>()
        .join(COLUMN_GAP)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::{Priority, Risk, TaskMetadata, TaskStatus, TaskType};

    use super::*;

    #[test]
    fn task_table_aligns_dynamic_columns_and_all_enum_values() {
        let statuses = [
            TaskStatus::ToDo,
            TaskStatus::InProgress,
            TaskStatus::Blocked,
            TaskStatus::Done,
            TaskStatus::Deferred,
            TaskStatus::Obsolete,
        ];
        let types = [
            "Milestone",
            "Epic",
            "Feature",
            "Bug",
            "Refactor",
            "TechDebt",
            "TestDebt",
            "Design",
            "Docs",
        ]
        .map(TaskType::new);
        let priorities = [Priority::High, Priority::Medium, Priority::Low];
        let risks = [Some(Risk::High), Some(Risk::Medium), Some(Risk::Low), None];
        let tasks: Vec<Task> = types
            .iter()
            .enumerate()
            .map(|(index, task_type)| {
                task(
                    if index == 0 {
                        "X"
                    } else {
                        "LONG-DYNAMIC-IDENTIFIER"
                    },
                    statuses[index % statuses.len()],
                    task_type.clone(),
                    priorities[index % priorities.len()],
                    risks[index % risks.len()],
                    if index % 2 == 0 { "core" } else { "platform" },
                    (index % 2 == 0).then_some("0.3.0"),
                    "Title",
                )
            })
            .collect();

        let output = render_tasks(&tasks);
        let lines: Vec<&str> = output.lines().collect();
        let header_positions: Vec<usize> = TASK_HEADERS
            .iter()
            .map(|header| lines[0].find(header).expect("header should exist"))
            .collect();

        assert_eq!(lines.len(), tasks.len() + 2);
        assert!(
            lines[1]
                .chars()
                .all(|character| character == '-' || character == ' ')
        );
        for (line, task) in lines[2..].iter().zip(&tasks) {
            let values = [
                task.qualified_id(),
                task.metadata.status.to_string(),
                task.metadata.task_type.to_string(),
                task.metadata.priority.to_string(),
                task.metadata
                    .risk
                    .map(|risk| risk.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                task.domain.clone(),
                task.metadata
                    .milestone
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                task.metadata.title.clone(),
            ];
            for (position, value) in header_positions.iter().zip(values) {
                assert!(line[*position..].starts_with(&value));
            }
        }
        assert!(output.contains("To Do"));
        assert!(output.contains("In Progress"));
        assert!(output.contains("TechDebt"));
        assert!(output.contains("TestDebt"));
        assert!(output.contains("  -         "));
    }

    #[test]
    fn task_table_truncates_long_utf8_titles_at_48_characters() {
        let title = "界".repeat(49);
        let output = render_tasks(&[task(
            "CORE-001",
            TaskStatus::ToDo,
            TaskType::new("Feature"),
            Priority::High,
            None,
            "core",
            None,
            &title,
        )]);
        let rendered_title = output.lines().nth(2).expect("task row should exist");
        let rendered_title = rendered_title
            .split(COLUMN_GAP)
            .last()
            .expect("title column should exist");

        assert_eq!(rendered_title.chars().count(), MAX_TITLE_CHARS);
        assert!(rendered_title.ends_with("..."));
        assert_eq!(
            rendered_title.chars().take(45).collect::<String>(),
            "界".repeat(45)
        );
    }

    #[test]
    fn empty_task_table_has_no_header() {
        assert_eq!(render_tasks(&[]), "No tasks.");
    }

    #[test]
    fn summary_uses_shared_label_and_count_widths() {
        let status = BTreeMap::from([("To Do".to_string(), 2)]);
        let domain = BTreeMap::from([("long-domain".to_string(), 12)]);
        let priority = BTreeMap::from([("High".to_string(), 1)]);

        assert_eq!(
            render_summary(&[
                ("Status", &status),
                ("Domain", &domain),
                ("Priority", &priority),
            ]),
            "Status\nTo Do      :  2\n\nDomain\nlong-domain: 12\n\nPriority\nHigh       :  1"
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn task(
        id: &str,
        status: TaskStatus,
        task_type: TaskType,
        priority: Priority,
        risk: Option<Risk>,
        domain: &str,
        milestone: Option<&str>,
        title: &str,
    ) -> Task {
        Task {
            path: PathBuf::from("task.md"),
            domain: domain.to_string(),
            metadata: TaskMetadata {
                id: id.to_string(),
                title: title.to_string(),
                status,
                priority,
                task_type,
                parent: None,
                milestone: milestone.map(str::to_string),
                depends_on: Vec::new(),
                rules: Vec::new(),
                risk,
                impact: None,
                tags: Vec::new(),
                whitepaper: None,
                last_updated: None,
            },
        }
    }
}
