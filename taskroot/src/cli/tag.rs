//! # Tag Group Commands
//!
//! These commands change one exclusive tag group without making callers merge
//! the task's full tag list. The lifecycle module keeps validation and atomic
//! writes behind the command interface.
//!
//! ```
//! use clap::Parser;
//! use taskroot::cli::Cli;
//!
//! let parsed = Cli::try_parse_from([
//!     "taskroot",
//!     "tag",
//!     "clear",
//!     "core:CORE-001",
//!     "triage-state",
//! ]);
//! assert!(parsed.is_ok());
//! ```

use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::{ROOT_HELP, resolve_existing_selector, resolve_project};
use crate::{
    error::TaskError,
    lifecycle::set_exclusive_tag_group_value,
    output::{TaskAction, TaskActionOutput, render_task_action},
};

#[derive(Debug, Args)]
pub struct TagArgs {
    #[command(subcommand)]
    command: TagCommand,
}

#[derive(Debug, Subcommand)]
enum TagCommand {
    Set(TagSetArgs),
    Clear(TagClearArgs),
}

#[derive(Debug, Args)]
struct TagSetArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    root: Option<PathBuf>,
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
    id: String,
    #[arg(value_name = "GROUP", help = "Configured exclusive tag group")]
    group: String,
    #[arg(value_name = "VALUE", help = "Tag value configured in the group")]
    value: String,
}

#[derive(Debug, Args)]
struct TagClearArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    root: Option<PathBuf>,
    #[arg(value_name = "TASK", help = "Exact domain:LOCAL-ID task identity")]
    id: String,
    #[arg(value_name = "GROUP", help = "Configured exclusive tag group")]
    group: String,
}

pub fn run_tag(args: TagArgs) -> Result<(), TaskError> {
    match args.command {
        TagCommand::Set(args) => {
            run_tag_update(args.root, &args.id, &args.group, Some(&args.value))
        }
        TagCommand::Clear(args) => run_tag_update(args.root, &args.id, &args.group, None),
    }
}

fn run_tag_update(
    root: Option<PathBuf>,
    id: &str,
    group: &str,
    value: Option<&str>,
) -> Result<(), TaskError> {
    let project = resolve_project(root)?;
    let identity = resolve_existing_selector(&project, id)?;
    let path = set_exclusive_tag_group_value(&project, &identity, group, value)?;
    println!(
        "{}",
        render_task_action(&TaskActionOutput {
            action: TaskAction::TagsUpdated,
            task_id: &identity.to_string(),
            root: project.root(),
            path: &path,
        })
    );
    Ok(())
}
