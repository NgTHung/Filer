//! # Read Routes
//!
//! List and detail endpoints validate the selected repository per request so
//! external CLI and git edits are visible without serving cached task data.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use filer_task::{
    agent_context::{ShowView, build_show},
    graph::TaskGraph,
    model::{Priority, SortBy, Task, TaskStatus},
    project::TaskProject,
    reference::IdentityIndex,
    validate::{TaskFilter, filter_tasks},
};
use serde::Deserialize;

use crate::{app::AppState, dto::ProjectSummary, error::WebError, routes::blocking};

#[derive(Debug, Deserialize)]
pub struct TaskQuery {
    status: Option<String>,
    priority: Option<String>,
    domain: Option<String>,
    parent: Option<String>,
    milestone: Option<String>,
    tag: Option<String>,
    blocked: Option<bool>,
    sort_by: Option<String>,
}

pub(crate) async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectSummary>>, WebError> {
    let registry = state.registry.clone();
    let projects = blocking(move || registry.summaries()).await?;
    Ok(Json(projects))
}

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<Vec<Task>>, WebError> {
    let registered = state.registry.resolve(&project_name)?.clone();
    let sort_by = parse_sort(query.sort_by.as_deref())?;
    let tasks = blocking(move || {
        let validated = registered.validate()?;
        let filter = build_filter(registered.task_project(), &validated.tasks, &query)?;
        let graph = TaskGraph::new(registered.task_project(), &validated.tasks)?;
        Ok(filter_tasks(
            registered.task_project(),
            &validated.tasks,
            &graph,
            &filter,
            sort_by,
        )?)
    })
    .await?;
    Ok(Json(tasks))
}

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path((project_name, id)): Path<(String, String)>,
) -> Result<Json<ShowView>, WebError> {
    let registered = state.registry.resolve(&project_name)?.clone();
    let view = blocking(move || {
        let validated = registered.validate()?;
        let identity = resolve_identity(registered.task_project(), &validated.tasks, &id)?;
        Ok(build_show(
            registered.task_project(),
            &validated.tasks,
            &identity,
            &validated.warnings,
        )?)
    })
    .await?;
    Ok(Json(view))
}

fn build_filter(
    project: &TaskProject,
    tasks: &[Task],
    query: &TaskQuery,
) -> Result<TaskFilter, WebError> {
    let parent = query
        .parent
        .as_deref()
        .map(|value| resolve_identity(project, tasks, value))
        .transpose()?;
    Ok(TaskFilter {
        status: parse_opt::<TaskStatus>(query.status.as_deref())?,
        priority: parse_opt::<Priority>(query.priority.as_deref())?,
        domain: query.domain.clone(),
        parent,
        milestone: query.milestone.clone(),
        tag: query.tag.clone(),
        blocked: query.blocked.unwrap_or(false),
    })
}

pub(crate) fn resolve_identity(
    project: &TaskProject,
    tasks: &[Task],
    value: &str,
) -> Result<filer_task::identity::TaskIdentity, WebError> {
    let index = IdentityIndex::new(project.root(), tasks.iter().map(Task::identity));
    Ok(index.resolve_cli(value)?)
}

fn parse_opt<T: std::str::FromStr<Err = String>>(
    value: Option<&str>,
) -> Result<Option<T>, WebError> {
    match value {
        Some(raw) => raw.parse::<T>().map(Some).map_err(WebError::BadRequest),
        None => Ok(None),
    }
}

fn parse_sort(value: Option<&str>) -> Result<SortBy, WebError> {
    match value {
        None | Some("Id") => Ok(SortBy::Id),
        Some("Status") => Ok(SortBy::Status),
        Some("Priority") => Ok(SortBy::Priority),
        Some("Domain") => Ok(SortBy::Domain),
        Some(other) => Err(WebError::BadRequest(format!("invalid sort_by: {other}"))),
    }
}
