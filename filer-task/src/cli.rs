use std::{fs, path::PathBuf};

use crate::{
    agent_context::{ReadyFilter, build_context, build_ready, build_show},
    error::TaskError,
    identity::{IdentityError, TaskIdentity},
    lifecycle::{
        Criterion, NewTask, add_task, block_task, defer_task, done_task, import_tasks,
        obsolete_task, start_task,
    },
    markdown::checklist_items,
    model::{Priority, Risk, SortBy, Task, TaskStatus, TaskType},
    output::{
        ImportOutput, MilestoneOutput, SummaryOutput, TaskAction, TaskActionOutput,
        ValidationOutput, render_context, render_import, render_milestone, render_ready,
        render_show, render_summary_output, render_task_action, render_tasks, render_validation,
    },
    project::TaskProject,
    repo::discover_project_root,
    validate::{TaskFilter, filter_tasks, require_valid_report, validate_repo},
};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};

const ROOT_HELP: &str = "Start project discovery at PATH; defaults to the current directory and accepts paths nested inside a project";

#[derive(Debug, Parser)]
#[command(name = "filer-task")]
#[command(about = "Manage project task files")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate(ValidateArgs),
    List(ListArgs),
    Show(DetailArgs),
    Ready(ReadyArgs),
    Context(DetailArgs),
    Deps(DepsArgs),
    Milestone(MilestoneArgs),
    Summary(SummaryArgs),
    Add(AddArgs),
    Import(ImportArgs),
    Start(TaskIdArgs),
    Done(TaskIdArgs),
    Block(ReasonArgs),
    Defer(ReasonArgs),
    Obsolete(ReasonArgs),
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
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
pub struct DetailArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    pub id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct ReadyArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub domain: Option<String>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(long)]
    pub priority: Option<Priority>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct DepsArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub incomplete: bool,
    pub id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct MilestoneArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    pub milestone: String,
    #[arg(long)]
    pub exit_checklist: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct SummaryArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(
        long,
        help = "Task domain; required when --id is not qualified as domain:LOCAL-ID"
    )]
    pub domain: Option<String>,
    #[arg(
        long,
        help = "Local task ID with --domain, or a qualified domain:LOCAL-ID"
    )]
    pub id: String,
    #[arg(long)]
    pub title: String,
    #[arg(long, default_value = "To Do")]
    pub status: TaskStatus,
    #[arg(long)]
    pub priority: Priority,
    #[arg(long = "type", value_name = "TYPE")]
    pub task_type: TaskType,
    #[arg(long)]
    pub parent: Option<String>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(long = "depends-on", value_delimiter = ',')]
    pub depends_on: Vec<String>,
    #[arg(long = "rule", value_delimiter = ',')]
    pub rules: Vec<String>,
    #[arg(long)]
    pub risk: Option<Risk>,
    #[arg(long)]
    pub impact: Option<String>,
    #[arg(long = "tag", value_delimiter = ',')]
    pub tags: Vec<String>,
    #[arg(long)]
    pub whitepaper: Option<String>,
    #[arg(long)]
    pub summary: Option<String>,
    #[arg(long = "criterion")]
    pub criteria: Vec<String>,
    #[arg(long = "checked-criterion")]
    pub checked_criteria: Vec<String>,
    #[arg(long)]
    pub rationale: Option<String>,
    #[arg(long = "blocked-reason")]
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    pub file: PathBuf,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub skip_existing: bool,
}

#[derive(Debug, Args)]
pub struct TaskIdArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Args)]
pub struct ReasonArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
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
    let project = resolve_project(args.root)?;
    let report = validate_repo(&project)?;
    let task_count = report.tasks.len();
    require_valid_report(report)?;
    println!("{}", render_validation(&ValidationOutput { task_count }));
    Ok(())
}

pub fn run_list(args: ListArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let report = validate_repo(&project)?;
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
        OutputFormat::Human => println!("{}", render_tasks(&filtered)),
        OutputFormat::Json => print_json(&filtered)?,
    }

    Ok(())
}

pub fn run_show(args: DetailArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
    let view = build_show(&project, &tasks, &args.id)?;
    match args.format {
        OutputFormat::Human => println!("{}", render_show(&view)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&view)?),
    }
    Ok(())
}

pub fn run_ready(args: ReadyArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
    let view = build_ready(
        &project,
        &tasks,
        &ReadyFilter {
            domain: args.domain,
            milestone: args.milestone,
            priority: args.priority,
            tag: args.tag,
            limit: args.limit,
        },
    );
    match args.format {
        OutputFormat::Human => println!("{}", render_ready(&view)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&view)?),
    }
    Ok(())
}

pub fn run_context(args: DetailArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
    let view = build_context(&project, &tasks, &args.id)?;
    match args.format {
        OutputFormat::Human => println!("{}", render_context(&view)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&view)?),
    }
    Ok(())
}

pub fn run_deps(args: DepsArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
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
        OutputFormat::Human => println!("{}", render_tasks(&deps)),
        OutputFormat::Json => print_json(&deps)?,
    }
    Ok(())
}

pub fn run_milestone(args: MilestoneArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
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
            println!(
                "{}",
                render_milestone(&MilestoneOutput {
                    milestone: &args.milestone,
                    title: &milestone.metadata.title,
                    summary: &summary,
                    exit_criteria: args.exit_checklist.then_some(criteria.as_slice()),
                    open_tasks: &open_tasks,
                })
            );
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
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
    let scoped = match args.milestone {
        Some(milestone) => milestone_tasks(&tasks, &milestone),
        None => tasks,
    };
    let summary = build_summary(&scoped);
    match args.format {
        OutputFormat::Human => println!("{}", render_summary_output(&summary)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&summary)?),
    }
    Ok(())
}

pub fn run_add(args: AddArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_add_identity(&project, args.domain.as_deref(), &args.id)?;
    let TaskIdentity {
        domain,
        id: task_id,
    } = identity;
    let path = add_task(
        &project,
        NewTask {
            domain,
            id: task_id.clone(),
            title: args.title,
            status: args.status,
            priority: args.priority,
            task_type: args.task_type,
            parent: args.parent,
            milestone: args.milestone,
            depends_on: args.depends_on,
            rules: args.rules,
            risk: args.risk,
            impact: args.impact,
            tags: args.tags,
            whitepaper: args.whitepaper,
            summary: args.summary,
            criteria: criteria_from_args(args.criteria, args.checked_criteria),
            rationale: args.rationale,
            blocked_reason: args.blocked_reason,
        },
    )?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Created,
            task_id: &task_id,
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

fn resolve_add_identity(
    project: &TaskProject,
    domain: Option<&str>,
    id: &str,
) -> Result<TaskIdentity, TaskError> {
    let identity = if id.contains(':') {
        let identity =
            TaskIdentity::parse(id).map_err(|error| invalid_reference(project, id, &error))?;
        if let Some(domain) = domain
            && identity.domain != domain
        {
            return Err(TaskError::DomainConflict {
                identity_domain: identity.domain.clone(),
                flag_domain: domain.to_string(),
                root: project.root().to_path_buf(),
            });
        }
        identity
    } else {
        let domain = domain.ok_or_else(|| TaskError::DomainRequired {
            id: id.to_string(),
            root: project.root().to_path_buf(),
        })?;
        TaskIdentity::new(domain, id)
            .map_err(|error| invalid_reference(project, error.value(), &error))?
    };

    if project.policy().domain(&identity.domain).is_none() {
        return Err(TaskError::UnknownDomain {
            domain: identity.domain.clone(),
            configured: project.policy().domains().keys().cloned().collect(),
            root: project.root().to_path_buf(),
        });
    }
    Ok(identity)
}

fn invalid_reference(project: &TaskProject, reference: &str, error: &IdentityError) -> TaskError {
    TaskError::InvalidReference {
        reference: reference.to_string(),
        constraint: error.constraint().to_string(),
        root: project.root().to_path_buf(),
    }
}

pub fn run_import(args: ImportArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let content = fs::read_to_string(&args.file).map_err(|source| TaskError::Io {
        path: args.file.clone(),
        source,
    })?;
    let tasks: Vec<ImportTask> = serde_json::from_str(&content)?;
    let tasks: Vec<NewTask> = tasks.into_iter().map(NewTask::from).collect();
    let paths = import_tasks(&project, &tasks, args.dry_run, args.skip_existing)?;
    println!(
        "{}",
        render_import(&ImportOutput {
            dry_run: args.dry_run,
            root: project.root(),
            paths: &paths,
        })
    );
    Ok(())
}

pub fn run_start(args: TaskIdArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let path = start_task(&project, &args.id)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Started,
            task_id: &args.id,
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_done(args: TaskIdArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let path = done_task(&project, &args.id)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Completed,
            task_id: &args.id,
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_block(args: ReasonArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let path = block_task(&project, &args.id, &args.reason)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Blocked,
            task_id: &args.id,
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_defer(args: ReasonArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let path = defer_task(&project, &args.id, &args.reason)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Deferred,
            task_id: &args.id,
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_obsolete(args: ReasonArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let path = obsolete_task(&project, &args.id, &args.reason)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Obsolete,
            task_id: &args.id,
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

fn resolve_project(root: Option<PathBuf>) -> Result<TaskProject, TaskError> {
    let start = match root {
        Some(root) if root.is_absolute() => root,
        Some(root) => current_working_directory()?.join(root),
        None => current_working_directory()?,
    };
    let root = discover_project_root(start)?;
    TaskProject::open(root)
}

fn current_working_directory() -> Result<PathBuf, TaskError> {
    std::env::current_dir().map_err(|source| TaskError::Io {
        path: PathBuf::from("."),
        source,
    })
}

fn print_json(tasks: &[Task]) -> Result<(), TaskError> {
    let json = serde_json::to_string_pretty(tasks)?;
    println!("{json}");
    Ok(())
}

#[derive(Debug, Serialize)]
struct MilestoneView<'a> {
    milestone: &'a Task,
    exit_criteria: Vec<crate::markdown::ChecklistItem>,
    counts: SummaryOutput,
    open_tasks: Vec<Task>,
}

fn milestone_tasks(tasks: &[Task], milestone: &str) -> Vec<Task> {
    tasks
        .iter()
        .filter(|task| task.metadata.milestone.as_deref() == Some(milestone))
        .cloned()
        .collect()
}

fn build_summary(tasks: &[Task]) -> SummaryOutput {
    let mut summary = SummaryOutput {
        status: Default::default(),
        domain: Default::default(),
        priority: Default::default(),
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

#[derive(Debug, Deserialize)]
struct ImportTask {
    domain: String,
    id: String,
    title: String,
    #[serde(default = "default_status")]
    status: TaskStatus,
    priority: Priority,
    #[serde(rename = "type")]
    task_type: TaskType,
    parent: Option<String>,
    milestone: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    rules: Vec<String>,
    risk: Option<Risk>,
    impact: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    whitepaper: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    criteria: Vec<Criterion>,
    rationale: Option<String>,
    blocked_reason: Option<String>,
}

impl From<ImportTask> for NewTask {
    fn from(value: ImportTask) -> Self {
        Self {
            domain: value.domain,
            id: value.id,
            title: value.title,
            status: value.status,
            priority: value.priority,
            task_type: value.task_type,
            parent: value.parent,
            milestone: value.milestone,
            depends_on: value.depends_on,
            rules: value.rules,
            risk: value.risk,
            impact: value.impact,
            tags: value.tags,
            whitepaper: value.whitepaper,
            summary: value.summary,
            criteria: value.criteria,
            rationale: value.rationale,
            blocked_reason: value.blocked_reason,
        }
    }
}

fn default_status() -> TaskStatus {
    TaskStatus::ToDo
}

fn criteria_from_args(open: Vec<String>, checked: Vec<String>) -> Vec<Criterion> {
    open.into_iter()
        .map(|text| Criterion {
            text,
            checked: false,
        })
        .chain(checked.into_iter().map(|text| Criterion {
            text,
            checked: true,
        }))
        .collect()
}
