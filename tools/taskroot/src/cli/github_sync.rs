//! # GitHub Project Publication
//!
//! Publication keeps local tasks authoritative and requires an explicit apply
//! flag so reviewing a plan cannot change a remote project.
//!
//! ```
//! use clap::Parser;
//! use taskroot::cli::Cli;
//! assert!(Cli::try_parse_from([
//!     "taskroot", "github-sync", "--project", "PVT_example",
//!     "--repository", "owner/repo",
//! ]).is_ok());
//! ```

use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::Args;
use serde::Serialize;
use serde_json::{Value, json};

use super::{OutputFormat, ROOT_HELP, resolve_project};
use crate::{
    error::TaskError,
    frontmatter::body_after_frontmatter,
    validate::{require_valid_report, validate_repo},
};

mod api;

#[derive(Debug, Args)]
pub struct GithubSyncArgs {
    #[arg(long, value_name = "PATH", help = ROOT_HELP)]
    root: Option<PathBuf>,
    #[arg(long, help = "GitHub Project node ID (PVT_...)")]
    project: String,
    #[arg(
        long,
        help = "Source repository OWNER/REPO, also used as the ownership namespace"
    )]
    repository: String,
    #[arg(long, help = "Publish the previewed changes; default is read-only")]
    apply: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
}

#[derive(Serialize)]
struct Change {
    qualified_id: String,
    action: &'static str,
    #[serde(skip)]
    title: String,
    #[serde(skip)]
    body: String,
    #[serde(skip)]
    remote: Option<Value>,
}

pub fn run_github_sync(args: GithubSyncArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let tasks = require_valid_report(validate_repo(&project)?)?;
    let parts: Vec<_> = args.repository.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        })
    {
        return Err(api::invalid("repository must be OWNER/REPO"));
    }
    let items = api::items(&args.project)?;
    let mut remote = BTreeMap::new();
    let prefix = format!("<!-- taskroot:{}:", args.repository);
    for item in items {
        let content = &item["content"];
        let body = content["body"].as_str().unwrap_or_default();
        if let Some(key) = body
            .lines()
            .find_map(|line| {
                line.strip_prefix(&prefix)
                    .and_then(|v| v.strip_suffix(" -->"))
            })
            .map(str::to_owned)
        {
            if content["__typename"] != "DraftIssue" || item["isArchived"] == true {
                return Err(api::invalid(format!(
                    "{key} is archived or converted; restore its draft before syncing"
                )));
            }
            if remote.insert(key.to_string(), item).is_some() {
                return Err(api::invalid(format!(
                    "duplicate taskroot ownership marker for {key}"
                )));
            }
        }
    }
    let mut changes = Vec::new();
    for task in &tasks {
        let qualified_id = task.qualified_id();
        let content = fs::read_to_string(&task.path).map_err(|source| TaskError::Io {
            path: task.path.clone(),
            source,
        })?;
        let body = format!(
            "{prefix}{qualified_id} -->\n\nManaged by taskroot from {}. Local tasks are authoritative.\n\nStatus: {} | Priority: {} | Domain: {} | Milestone: {}\n\n{}",
            args.repository,
            task.metadata.status,
            task.metadata.priority,
            task.domain,
            task.metadata.milestone.as_deref().unwrap_or("-"),
            body_after_frontmatter(&content)?.trim()
        );
        let title = format!("[{qualified_id}] {}", task.metadata.title);
        let existing = remote.remove(&qualified_id);
        let action = match &existing {
            None => "create",
            Some(item) if item["content"]["title"] == title && item["content"]["body"] == body => {
                "unchanged"
            }
            Some(_) => "update",
        };
        changes.push(Change {
            qualified_id,
            action,
            title,
            body,
            remote: existing,
        });
    }
    if args.apply {
        for change in changes.iter().filter(|c| c.action != "unchanged") {
            match &change.remote {
                None => {
                    let created = api::request(
                        "mutation($input:AddProjectV2DraftIssueInput!){addProjectV2DraftIssue(input:$input){projectItem{id}}}",
                        json!({"input":{"projectId":args.project,"title":change.title,"body":change.body}}),
                    )?;
                    if !created["addProjectV2DraftIssue"]["projectItem"]["id"].is_string() {
                        return Err(api::invalid(
                            "missing created draft ID; rerun to reconcile before retrying",
                        ));
                    }
                }
                Some(item) => {
                    let draft = item["content"]["id"]
                        .as_str()
                        .ok_or_else(|| api::invalid("missing draft content ID"))?;
                    api::request(
                        "mutation($input:UpdateProjectV2DraftIssueInput!){updateProjectV2DraftIssue(input:$input){draftIssue{id}}}",
                        json!({"input":{"draftIssueId":draft,"title":change.title,"body":change.body}}),
                    )?;
                }
            }
        }
    }
    let result = json!({
        "mode": if args.apply { "apply" } else { "preview" },
        "create": changes.iter().filter(|c| c.action == "create").count(),
        "update": changes.iter().filter(|c| c.action == "update").count(),
        "unchanged": changes.iter().filter(|c| c.action == "unchanged").count(),
        "tasks": changes,
    });
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Human => {
            println!(
                "GitHub Sync\nMode: {}\nCreate: {}\nUpdate: {}\nUnchanged: {}",
                result["mode"].as_str().unwrap_or_default(),
                result["create"],
                result["update"],
                result["unchanged"]
            );
            for change in changes.iter().filter(|c| c.action != "unchanged") {
                println!("{} {}", change.action, change.qualified_id);
            }
        }
    }
    Ok(())
}
