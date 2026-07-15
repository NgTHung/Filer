use std::{fs, path::PathBuf};

use crate::{
    agent_context::{ReadyFilter, build_context, build_ready, build_show},
    error::TaskError,
    graph::TaskGraph,
    identity::{IdentityError, TaskIdentity},
    lifecycle::{
        Criterion, NewTask, add_task, block_task, defer_task, done_task, import_tasks,
        obsolete_task, start_task, toggle_criterion,
    },
    markdown::checklist_items,
    milestone::tasks_for_milestone,
    model::{Priority, Risk, SortBy, Task, TaskStatus, TaskType},
    output::{
        ImportOutput, MilestoneOutput, SummaryOutput, TaskAction, TaskActionOutput,
        ValidationOutput, render_context, render_import, render_milestone, render_ready,
        render_show, render_summary_output, render_task_action, render_tasks, render_validation,
    },
    project::{InitDomain, InitProjectOptions, TaskProject},
    reference::IdentityIndex,
    repo::discover_project_root,
    taxonomy::{criteria_heading, is_milestone_type},
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
    Init(InitArgs),
    Add(Box<AddArgs>),
    Import(ImportArgs),
    Start(TaskIdArgs),
    Done(TaskIdArgs),
    CriterionToggle(CriterionToggleArgs),
    Block(ReasonArgs),
    Defer(ReasonArgs),
    Obsolete(ReasonArgs),
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
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
    #[arg(
        long,
        value_name = "TASK",
        help = "Filter by an exact domain:LOCAL-ID parent identity"
    )]
    pub parent: Option<String>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(long, help = "Filter by a tag accepted by the project tag policy")]
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
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
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
    #[arg(long, help = "Filter by a tag accepted by the project tag policy")]
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
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
    pub id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct MilestoneArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(help = "Milestone value bound to the configured milestone-role task")]
    pub milestone: String,
    #[arg(long, help = "Show the milestone type's configured criteria checklist")]
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
pub struct InitArgs {
    #[arg(
        long,
        value_name = "PATH",
        help = "Directory where .tasks will be created; defaults to the current directory"
    )]
    pub root: Option<PathBuf>,
    #[arg(long, default_value = "default")]
    pub domain: String,
    #[arg(long = "prefix", value_delimiter = ',', default_value = "WORK")]
    pub prefixes: Vec<String>,
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
    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Task type name declared in .tasks/config.json"
    )]
    pub task_type: TaskType,
    #[arg(
        long,
        help = "Local parent in the new task's domain, or a cross-domain domain:LOCAL-ID"
    )]
    pub parent: Option<String>,
    #[arg(long)]
    pub milestone: Option<String>,
    #[arg(
        long = "depends-on",
        value_delimiter = ',',
        help = "Local dependencies in the new task's domain or qualified cross-domain identities"
    )]
    pub depends_on: Vec<String>,
    #[arg(long = "rule", value_delimiter = ',')]
    pub rules: Vec<String>,
    #[arg(long)]
    pub risk: Option<Risk>,
    #[arg(long)]
    pub impact: Option<String>,
    #[arg(
        long = "tag",
        value_delimiter = ',',
        help = "Portable tag accepted by the project tag policy"
    )]
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
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
    pub id: String,
}

#[derive(Debug, Args)]
pub struct CriterionToggleArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
    pub id: String,
    #[arg(value_name = "INDEX", help = "Zero-based criteria index")]
    pub index: usize,
}

#[derive(Debug, Args)]
pub struct ReasonArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    pub root: Option<PathBuf>,
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
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
    let validated = require_valid_report(report)?;
    let output = ValidationOutput {
        task_count,
        warnings: &validated.warnings,
    };
    match args.format {
        OutputFormat::Human => println!("{}", render_validation(&output)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&output)?),
    }
    Ok(())
}

pub fn run_list(args: ListArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let report = validate_repo(&project)?;
    let validated = require_valid_report(report)?;
    let parent = args
        .parent
        .as_deref()
        .map(|value| resolve_selector(&project, &validated.tasks, value))
        .transpose()?;
    let graph = TaskGraph::new(&project, &validated.tasks)?;
    let filtered = filter_tasks(
        &project,
        &validated.tasks,
        &graph,
        &TaskFilter {
            status: args.status,
            priority: args.priority,
            domain: args.domain,
            parent,
            milestone: args.milestone,
            tag: args.tag,
            blocked: args.blocked,
        },
        args.sort_by,
    )?;

    match args.format {
        OutputFormat::Human => println!("{}", render_tasks(&filtered)),
        OutputFormat::Json => print_json(&filtered)?,
    }

    Ok(())
}

pub fn run_show(args: DetailArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let validated = require_valid_report(validate_repo(&project)?)?;
    let identity = resolve_selector(&project, &validated.tasks, &args.id)?;
    let view = build_show(&project, &validated.tasks, &identity, &validated.warnings)?;
    match args.format {
        OutputFormat::Human => println!("{}", render_show(&view)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&view)?),
    }
    Ok(())
}

pub fn run_ready(args: ReadyArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let validated = require_valid_report(validate_repo(&project)?)?;
    let view = build_ready(
        &project,
        &validated.tasks,
        &ReadyFilter {
            domain: args.domain,
            milestone: args.milestone,
            priority: args.priority,
            tag: args.tag,
            limit: args.limit,
        },
        &validated.warnings,
    )?;
    match args.format {
        OutputFormat::Human => println!("{}", render_ready(&view)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&view)?),
    }
    Ok(())
}

pub fn run_context(args: DetailArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let validated = require_valid_report(validate_repo(&project)?)?;
    let identity = resolve_selector(&project, &validated.tasks, &args.id)?;
    let view = build_context(&project, &validated.tasks, &identity, &validated.warnings)?;
    match args.format {
        OutputFormat::Human => println!("{}", render_context(&view)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&view)?),
    }
    Ok(())
}

pub fn run_deps(args: DepsArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let validated = require_valid_report(validate_repo(&project)?)?;
    let identity = resolve_selector(&project, &validated.tasks, &args.id)?;
    let graph = TaskGraph::new(&project, &validated.tasks)?;
    let deps: Vec<Task> = graph
        .dependencies(&identity)
        .into_iter()
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
            is_milestone_type(&project, &task.metadata.task_type)
                && task.metadata.milestone.as_deref() == Some(args.milestone.as_str())
        })
        .ok_or_else(|| {
            TaskError::Message(format!("milestone {} does not exist", args.milestone))
        })?;
    let content = std::fs::read_to_string(&milestone.path).map_err(|source| TaskError::Io {
        path: milestone.path.clone(),
        source,
    })?;
    let scoped: Vec<Task> = tasks_for_milestone(&tasks, &args.milestone)
        .cloned()
        .collect();
    let summary = build_summary(&scoped);
    let heading = criteria_heading(
        &project,
        &milestone.metadata.task_type,
        Some(&milestone.domain),
        Some(&milestone.qualified_id()),
    )?;
    let criteria = checklist_items(&content, heading);
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
                    criteria_heading: heading,
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
        Some(milestone) => tasks_for_milestone(&tasks, &milestone).cloned().collect(),
        None => tasks.tasks,
    };
    let summary = build_summary(&scoped);
    match args.format {
        OutputFormat::Human => println!("{}", render_summary_output(&summary)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&summary)?),
    }
    Ok(())
}

pub fn run_init(args: InitArgs) -> Result<(), TaskError> {
    let root = match args.root {
        Some(root) if root.is_absolute() => root,
        Some(root) => current_working_directory()?.join(root),
        None => current_working_directory()?,
    };
    let project = TaskProject::init(
        &root,
        InitProjectOptions {
            domain: InitDomain {
                name: args.domain,
                prefixes: args.prefixes,
            },
        },
    )?;
    println!(
        "Project Initialized\nRoot: {}\nConfig: {}",
        project.root().display(),
        project.root().join(crate::project::CONFIG_PATH).display()
    );
    Ok(())
}

pub fn run_add(args: AddArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_add_identity(&project, args.domain.as_deref(), &args.id)?;
    let TaskIdentity {
        domain,
        id: task_id,
    } = identity;
    let qualified_id = format!("{domain}:{task_id}");
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
            task_id: &qualified_id,
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
            candidates: Vec::new(),
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
    let identity = resolve_existing_selector(&project, &args.id)?;
    let path = start_task(&project, &identity)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Started,
            task_id: &identity.to_string(),
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_done(args: TaskIdArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_existing_selector(&project, &args.id)?;
    let path = done_task(&project, &identity)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Completed,
            task_id: &identity.to_string(),
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_criterion_toggle(args: CriterionToggleArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_existing_selector(&project, &args.id)?;
    let path = toggle_criterion(&project, &identity, args.index)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::CriterionToggled,
            task_id: &identity.to_string(),
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_block(args: ReasonArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_existing_selector(&project, &args.id)?;
    let path = block_task(&project, &identity, &args.reason)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Blocked,
            task_id: &identity.to_string(),
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_defer(args: ReasonArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_existing_selector(&project, &args.id)?;
    let path = defer_task(&project, &identity, &args.reason)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Deferred,
            task_id: &identity.to_string(),
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}

pub fn run_obsolete(args: ReasonArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let identity = resolve_existing_selector(&project, &args.id)?;
    let path = obsolete_task(&project, &identity, &args.reason)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::Obsolete,
            task_id: &identity.to_string(),
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

fn resolve_existing_selector(
    project: &TaskProject,
    value: &str,
) -> Result<TaskIdentity, TaskError> {
    let validated = require_valid_report(validate_repo(project)?)?;
    resolve_selector(project, &validated.tasks, value)
}

fn resolve_selector(
    project: &TaskProject,
    tasks: &[Task],
    value: &str,
) -> Result<TaskIdentity, TaskError> {
    IdentityIndex::new(project.root(), tasks.iter().map(Task::identity)).resolve_cli(value)
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
