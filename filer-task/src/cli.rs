use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::{
    error::TaskError,
    model::{Priority, SortBy, Task, TaskStatus},
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
    pub tag: Option<String>,
    #[arg(long, value_enum, default_value_t = SortBy::Id)]
    pub sort_by: SortBy,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
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
            tag: args.tag,
        },
        args.sort_by,
    );

    match args.format {
        OutputFormat::Human => print_human(&filtered),
        OutputFormat::Json => print_json(&filtered)?,
    }

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
        "{:<12}  {:<12}  {:<9}  {:<8}  {:<8}  {:<9}  TITLE",
        "ID", "STATUS", "TYPE", "PRIORITY", "RISK", "DOMAIN"
    );
    for task in tasks {
        let risk = task
            .metadata
            .risk
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<12}  {:<12}  {:<9}  {:<8}  {:<8}  {:<9}  {}",
            task.metadata.id,
            task.metadata.status,
            task.metadata.task_type,
            task.metadata.priority,
            risk,
            task.domain,
            task.metadata.title
        );
    }
}

fn print_json(tasks: &[Task]) -> Result<(), TaskError> {
    let json = serde_json::to_string_pretty(tasks)?;
    println!("{json}");
    Ok(())
}
