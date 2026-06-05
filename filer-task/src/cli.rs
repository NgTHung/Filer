use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::{
    error::TaskError,
    lifecycle::{NewTask, add_task, block_task, defer_task, done_task, obsolete_task, start_task},
    markdown::checklist_items,
    model::{Priority, SortBy, Task, TaskStatus, TaskType},
    repo::find_repo_root,
    validate::{TaskFilter, filter_tasks, require_valid_report, validate_repo},
};

#[derive(Debug, Parser)]
#[command(name = "filer-task")]
#[command(about = "Manage Filer task files")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate(ValidateArgs),
    List(ListArgs),
    Deps(DepsArgs),
    Milestone(MilestoneArgs),
    Summary(SummaryArgs),
    Add(AddArgs),
    Start(TaskIdArgs),
    Done(TaskIdArgs),
    Block(ReasonArgs),
    Defer(ReasonArgs),
    Obsolete(ReasonArgs),
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub status: Option<TaskStatus>,
    #[arg(long)]
    pub priority: Option<Priority>,
    #[arg(long)]
    pub domain: Option<String>,
    #[arg(long)]
    pub parent: Option<String>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub blocked: bool,
    #[arg(long, value_enum, default_value_t = SortBy::Id)]
    pub sort_by: SortBy,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct DepsArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub incomplete: bool,
    pub id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct MilestoneArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    pub milestone: String,
    #[arg(long)]
    pub exit_checklist: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct SummaryArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub domain: String,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub priority: Priority,
    #[arg(long = "type", value_name = "TYPE")]
    pub task_type: TaskType,
    #[arg(long)]
    pub milestone: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskIdArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Args)]
pub struct ReasonArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

pub fn run_validate(args: ValidateArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let report = validate_repo(&root)?;
    let task_count = report.tasks.len();
    require_valid_report(report)?;
    println!("task validation passed ({task_count} task(s))");
    Ok(())
}

pub fn run_list(args: ListArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let report = validate_repo(&root)?;
    let tasks = require_valid_report(report)?;
    let filtered = filter_tasks(
        tasks,
        &TaskFilter {
            status: args.status,
            priority: args.priority,
            domain: args.domain,
            parent: args.parent,
            milestone: args.milestone,
            tag: args.tag,
            blocked: args.blocked,
        },
        args.sort_by,
    );

    match args.format {
        OutputFormat::Human => print_human(&filtered),
        OutputFormat::Json => print_json(&filtered)?,
    }

    Ok(())
}

pub fn run_deps(args: DepsArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let tasks = require_valid_report(validate_repo(&root)?)?;
    let task = tasks
        .iter()
        .find(|task| task.metadata.id == args.id)
        .ok_or_else(|| TaskError::Message(format!("task {} does not exist", args.id)))?;
    let deps: Vec<Task> = task
        .metadata
        .depends_on
        .iter()
        .filter_map(|id| tasks.iter().find(|task| task.metadata.id == *id))
        .filter(|task| !args.incomplete || task.metadata.status != TaskStatus::Done)
        .cloned()
        .collect();

    match args.format {
        OutputFormat::Human => print_human(&deps),
        OutputFormat::Json => print_json(&deps)?,
    }
    Ok(())
}

pub fn run_milestone(args: MilestoneArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let tasks = require_valid_report(validate_repo(&root)?)?;
    let milestone = tasks
        .iter()
        .find(|task| {
            task.metadata.task_type == TaskType::Milestone
                && task.metadata.milestone.as_deref() == Some(args.milestone.as_str())
        })
        .ok_or_else(|| {
            TaskError::Message(format!("milestone {} does not exist", args.milestone))
        })?;
    let content = std::fs::read_to_string(&milestone.path).map_err(|source| TaskError::Io {
        path: milestone.path.clone(),
        source,
    })?;
    let scoped = milestone_tasks(&tasks, &args.milestone);
    let summary = build_summary(&scoped);
    let criteria = checklist_items(&content, "Exit Criteria");
    let open_tasks: Vec<Task> = scoped
        .iter()
        .filter(|task| task.metadata.status != TaskStatus::Done)
        .cloned()
        .collect();

    match args.format {
        OutputFormat::Human => {
            println!("Milestone {}: {}", args.milestone, milestone.metadata.title);
            print_summary_human(&summary);
            if args.exit_checklist {
                println!("\nExit Criteria");
                for item in &criteria {
                    let marker = if item.checked { "x" } else { " " };
                    println!("- [{marker}] {}", item.text);
                }
            }
            println!("\nOpen Tasks");
            print_human(&open_tasks);
        }
        OutputFormat::Json => {
            let view = MilestoneView {
                milestone,
                exit_criteria: criteria,
                counts: summary,
                open_tasks,
            };
            println!("{}", serde_json::to_string_pretty(&view)?);
        }
    }
    Ok(())
}

pub fn run_summary(args: SummaryArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let tasks = require_valid_report(validate_repo(&root)?)?;
    let scoped = match args.milestone {
        Some(milestone) => milestone_tasks(&tasks, &milestone),
        None => tasks,
    };
    let summary = build_summary(&scoped);
    match args.format {
        OutputFormat::Human => print_summary_human(&summary),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&summary)?),
    }
    Ok(())
}

pub fn run_add(args: AddArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let path = add_task(
        &root,
        NewTask {
            domain: args.domain,
            id: args.id,
            title: args.title,
            priority: args.priority,
            task_type: args.task_type,
            milestone: args.milestone,
        },
    )?;
    println!("created {}", path.display());
    Ok(())
}

pub fn run_start(args: TaskIdArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let path = start_task(&root, &args.id)?;
    println!("started {}", path.display());
    Ok(())
}

pub fn run_done(args: TaskIdArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let path = done_task(&root, &args.id)?;
    println!("completed {}", path.display());
    Ok(())
}

pub fn run_block(args: ReasonArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let path = block_task(&root, &args.id, &args.reason)?;
    println!("blocked {}", path.display());
    Ok(())
}

pub fn run_defer(args: ReasonArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let path = defer_task(&root, &args.id, &args.reason)?;
    println!("deferred {}", path.display());
    Ok(())
}

pub fn run_obsolete(args: ReasonArgs) -> Result<(), TaskError> {
    let root = resolve_root(args.root)?;
    let path = obsolete_task(&root, &args.id, &args.reason)?;
    println!("marked obsolete {}", path.display());
    Ok(())
}

fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf, TaskError> {
    match root {
        Some(root) => find_repo_root(root),
        None => {
            let cwd = std::env::current_dir().map_err(|source| TaskError::Io {
                path: PathBuf::from("."),
                source,
            })?;
            find_repo_root(cwd)
        }
    }
}

fn print_human(tasks: &[Task]) {
    println!(
        "{:<14}  {:<12}  {:<9}  {:<8}  {:<8}  {:<9}  {:<9}  TITLE",
        "ID", "STATUS", "TYPE", "PRIORITY", "RISK", "DOMAIN", "MILESTONE"
    );
    for task in tasks {
        let risk = task
            .metadata
            .risk
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let milestone = task.metadata.milestone.as_deref().unwrap_or("-");
        println!(
            "{:<14}  {:<12}  {:<9}  {:<8}  {:<8}  {:<9}  {:<9}  {}",
            task.metadata.id,
            task.metadata.status,
            task.metadata.task_type,
            task.metadata.priority,
            risk,
            task.domain,
            milestone,
            task.metadata.title
        );
    }
}

fn print_json(tasks: &[Task]) -> Result<(), TaskError> {
    let json = serde_json::to_string_pretty(tasks)?;
    println!("{json}");
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct SummaryView {
    pub status: BTreeMap<String, usize>,
    pub domain: BTreeMap<String, usize>,
    pub priority: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct MilestoneView<'a> {
    milestone: &'a Task,
    exit_criteria: Vec<crate::markdown::ChecklistItem>,
    counts: SummaryView,
    open_tasks: Vec<Task>,
}

fn milestone_tasks(tasks: &[Task], milestone: &str) -> Vec<Task> {
    tasks
        .iter()
        .filter(|task| task.metadata.milestone.as_deref() == Some(milestone))
        .cloned()
        .collect()
}

fn build_summary(tasks: &[Task]) -> SummaryView {
    let mut summary = SummaryView {
        status: BTreeMap::new(),
        domain: BTreeMap::new(),
        priority: BTreeMap::new(),
    };
    for task in tasks {
        *summary
            .status
            .entry(task.metadata.status.to_string())
            .or_default() += 1;
        *summary.domain.entry(task.domain.clone()).or_default() += 1;
        *summary
            .priority
            .entry(task.metadata.priority.to_string())
            .or_default() += 1;
    }
    summary
}

fn print_summary_human(summary: &SummaryView) {
    println!("Status");
    for (key, value) in &summary.status {
        println!("{key}: {value}");
    }
    println!("\nDomain");
    for (key, value) in &summary.domain {
        println!("{key}: {value}");
    }
    println!("\nPriority");
    for (key, value) in &summary.priority {
        println!("{key}: {value}");
    }
}
