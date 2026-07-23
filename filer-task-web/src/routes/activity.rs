//! # Activity Route
//!
//! Serves the write audit trail recorded by `write::mutate` and the
//! project/policy routes. Reads require no identity, matching every other
//! list endpoint in this API.

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::{
    app::AppState,
    dto::ActivityEntry,
    error::WebError,
    storage::ActivityFilter,
};

#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    project: Option<String>,
    task: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub(crate) async fn list_activity(
    State(state): State<AppState>,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<Vec<ActivityEntry>>, WebError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let rows = state
        .storage()
        .list_activity(ActivityFilter {
            project: query.project,
            task_id: query.task,
            limit,
            offset,
        })
        .await?;
    Ok(Json(rows.into_iter().map(ActivityEntry::from).collect()))
}
