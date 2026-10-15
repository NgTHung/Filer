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
mod fields;

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
    fields: Vec<fields::FieldChange>,
    #[serde(skip)]
    title: String,
    #[serde(skip)]
    body: String,
    #[serde(skip)]
    remote: Option<Value>,
}

pub fn run_github_sync(mut args: GithubSyncArgs) -> Result<(), TaskError> {
    let project = resolve_project(args.root)?;
    let (tasks, contents) = project.with_write_lock(|| {
        let tasks = require_valid_report(validate_repo(&project)?)?;
        let contents = tasks
            .iter()
            .map(|task| {
                fs::read_to_string(&task.path).map_err(|source| TaskError::Io {
                    path: task.path.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((tasks, contents))
    })?;
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
    args.repository.make_ascii_lowercase();
    let (items, fields) = api::items(&args.project)?;
    let mut remote = BTreeMap::new();
    let prefix = format!("<!-- taskroot:{}:", args.repository);
    for item in items {
        if item["type"] == "REDACTED" || item["content"].is_null() {
            return Err(api::invalid(
                "inaccessible project item; cannot safely reconcile ownership",
            ));
        }
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
            crate::identity::TaskIdentity::parse(&key)
                .map_err(|error| api::invalid(error.to_string()))?;
            if !item["id"].is_string() || !content["id"].is_string() {
                return Err(api::invalid("missing owned item IDs"));
            }
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
    for (task, content) in tasks.iter().zip(&contents) {
        let qualified_id = task.qualified_id();
        let body = format!(
            "{prefix}{qualified_id} -->\n\nManaged by taskroot from {}. Local tasks are authoritative.\n\nStatus: {} | Priority: {} | Domain: {} | Milestone: {}\n\n{}",
            args.repository,
            task.metadata.status,
            task.metadata.priority,
            task.domain,
            task.metadata.milestone.as_deref().unwrap_or("-"),
            body_after_frontmatter(content)?
                .trim()
                .replace("\r\n", "\n")
        );
        let title = format!("[{qualified_id}] {}", task.metadata.title);
        let existing = remote.remove(&qualified_id);
        let field_changes = fields::changes(&fields, task, existing.as_ref())?;
        let action = match &existing {
            None => "create",
            Some(item)
                if item["content"]["title"] == title
                    && item["content"]["body"] == body
                    && field_changes.is_empty() =>
            {
                "unchanged"
            }
            Some(_) => "update",
        };
        changes.push(Change {
            qualified_id,
            action,
            fields: field_changes,
            title,
            body,
            remote: existing,
        });
    }
    if args.apply {
        let mut writer = api::Writer::default();
        for change in changes.iter().filter(|c| c.action != "unchanged") {
            match &change.remote {
                None => {
                    let created = writer.write(
                        "mutation($input:AddProjectV2DraftIssueInput!){addProjectV2DraftIssue(input:$input){projectItem{id}}}",
                        json!({"input":{"projectId":args.project,"title":change.title,"body":change.body}}),
                    )?;
                    let item = created["addProjectV2DraftIssue"]["projectItem"]["id"]
                        .as_str()
                        .ok_or_else(|| {
                            api::invalid(
                                "missing created draft ID; rerun to reconcile before retrying",
                            )
                        })?;
                    fields::update(&args.project, item, &change.fields, None, &mut writer)?;
                }
                Some(item) => {
                    let draft = item["content"]["id"]
                        .as_str()
                        .ok_or_else(|| api::invalid("missing draft content ID"))?;
                    let content_changed = item["content"]["title"] != change.title
                        || item["content"]["body"] != change.body;
                    let patch = content_changed.then(
                        || json!({"draftIssueId":draft,"title":change.title,"body":change.body}),
                    );
                    fields::update(
                        &args.project,
                        item["id"]
                            .as_str()
                            .ok_or_else(|| api::invalid("missing project item ID"))?,
                        &change.fields,
                        patch,
                        &mut writer,
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
