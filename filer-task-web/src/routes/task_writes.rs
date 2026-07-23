//! # Task Write Routes
//!
//! Creation and field edits delegate rendering and validation to `filer-task`.
//! Criterion updates require an exact source-line hash so a stale checklist
//! index cannot target changed content.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, header::IF_MATCH},
};
use filer_task::{
    agent_context::ShowView,
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{NewTask, add_task, edit_task as apply_task_patch, set_criterion_checked},
    model::TaskStatus,
};

use crate::{
    app::AppState,
    dto::{CreateTaskRequest, EditTaskRequest, SetCriterionRequest},
    error::WebError,
    identity::Actor,
    routes::{
        tasks::{resolve_identity, resolve_project_wide_identity},
        write,
    },
};

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
    actor: Actor,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<ShowView>, WebError> {
    let id = format!("{}-{}", body.prefix, body.number);
    let identity_domain = body.domain.clone();
    let identity_id = id.clone();
    let task = NewTask {
        domain: body.domain,
        id,
        title: body.title,
        status: TaskStatus::ToDo,
        priority: body.priority,
        task_type: body.task_type,
        parent: None,
        milestone: body.milestone,
        depends_on: Vec::new(),
        rules: Vec::new(),
        risk: None,
        impact: None,
        tags: body.tags,
        whitepaper: None,
        summary: None,
        criteria: Vec::new(),
        rationale: None,
        blocked_reason: None,
    };
    let view = write::mutate(
        state,
        actor,
        project_name,
        "task.create",
        None,
        move |project, _| {
            let identity = TaskIdentity::new(identity_domain, identity_id).map_err(|error| {
                TaskError::InvalidReference {
                    reference: error.value().to_string(),
                    constraint: error.constraint().to_string(),
                    root: project.root().to_path_buf(),
                }
            })?;
            add_task(project, task)?;
            Ok(identity)
        },
    )
    .await?;
    Ok(Json(view))
}

pub(crate) async fn edit_task(
    State(state): State<AppState>,
    Path((project_name, id)): Path<(String, String)>,
    actor: Actor,
    Json(body): Json<EditTaskRequest>,
) -> Result<Json<ShowView>, WebError> {
    let patch = body.into();
    let view = write::mutate(
        state,
        actor,
        project_name,
        "task.edit",
        None,
        move |project, tasks| {
            let identity = resolve_project_wide_identity(project, tasks, &id)?;
            apply_task_patch(project, &identity, patch).map_err(WebError::TaskEdit)?;
            Ok(identity)
        },
    )
    .await?;
    Ok(Json(view))
}

pub(crate) async fn set_criterion(
    State(state): State<AppState>,
    Path((project_name, id, index)): Path<(String, String, usize)>,
    actor: Actor,
    headers: HeaderMap,
    Json(body): Json<SetCriterionRequest>,
) -> Result<Json<ShowView>, WebError> {
    let expected_hash = if_match(&headers)?;
    let detail = Some(format!("index {index} = {}", body.checked));
    let view = write::mutate(
        state,
        actor,
        project_name,
        "task.criterion",
        detail,
        move |project, tasks| {
            let identity = resolve_identity(project, tasks, &id)?;
            set_criterion_checked(project, &identity, index, &expected_hash, body.checked)?;
            Ok(identity)
        },
    )
    .await?;
    Ok(Json(view))
}

fn if_match(headers: &HeaderMap) -> Result<String, WebError> {
    let mut values = headers.get_all(IF_MATCH).iter();
    let value = values
        .next()
        .ok_or_else(|| WebError::PreconditionRequired("If-Match is required".to_string()))?;
    if values.next().is_some() {
        return Err(WebError::BadRequest(
            "If-Match must contain one quoted SHA-256 hash".to_string(),
        ));
    }
    let raw = value
        .to_str()
        .map_err(|_| WebError::BadRequest("If-Match is not valid ASCII".to_string()))?;
    let Some(hash) = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(WebError::BadRequest(
            "If-Match must be a quoted SHA-256 hash".to_string(),
        ));
    };
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WebError::BadRequest(
            "If-Match must contain 64 lowercase hexadecimal characters".to_string(),
        ));
    }
    Ok(hash.to_string())
}
