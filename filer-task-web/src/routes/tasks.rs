//! # Read Routes
//!
//! List and detail endpoints. Listing re-reads and validates the repo per
//! request so the response reflects edits made by the CLI or git, not a cache.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use filer_task::{
    agent_context::{ShowView, build_show},
    model::{Priority, SortBy, Task, TaskStatus},
    validate::{TaskFilter, filter_tasks, require_valid_report, validate_repo},
};
use serde::Deserialize;

use crate::{app::AppState, error::WebError, routes::blocking};

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

pub async fn list_projects(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(
        state
            .registry
            .names()
            .into_iter()
            .map(str::to_string)
            .collect(),
    )
}

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<Vec<Task>>, WebError> {
    let root = state.registry.resolve(None)?.to_path_buf();
    let filter = build_filter(&query)?;
    let sort_by = parse_sort(query.sort_by.as_deref())?;
    let tasks = blocking(move || {
        let tasks = require_valid_report(validate_repo(&root)?)?;
        Ok(filter_tasks(tasks, &filter, sort_by))
    })
    .await?;
    Ok(Json(tasks))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ShowView>, WebError> {
    let root = state.registry.resolve(None)?.to_path_buf();
    let view = blocking(move || {
        let tasks = require_valid_report(validate_repo(&root)?)?;
        build_show(&root, &tasks, &id)
    })
    .await?;
    Ok(Json(view))
}

fn build_filter(query: &TaskQuery) -> Result<TaskFilter, WebError> {
    Ok(TaskFilter {
        status: parse_opt::<TaskStatus>(query.status.as_deref())?,
        priority: parse_opt::<Priority>(query.priority.as_deref())?,
        domain: query.domain.clone(),
        parent: query.parent.clone(),
        milestone: query.milestone.clone(),
        tag: query.tag.clone(),
        blocked: query.blocked.unwrap_or(false),
    })
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
