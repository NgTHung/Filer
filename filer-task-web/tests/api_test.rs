use std::fs;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::app::{AppState, router};

#[tokio::test]
async fn list_returns_seeded_tasks() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");
    write_task(&repo, "CORE-002", "Caching", "In Progress", "Low");

    let (status, body) = get(&repo, "/api/tasks").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("array").len(), 2);
}

#[tokio::test]
async fn list_filters_by_status() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");
    write_task(&repo, "CORE-002", "Caching", "In Progress", "Low");

    let (status, body) = get(&repo, "/api/tasks?status=In%20Progress").await;

    assert_eq!(status, StatusCode::OK);
    let tasks = body.as_array().expect("array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "CORE-002");
}

#[tokio::test]
async fn get_task_returns_detail() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = get(&repo, "/api/tasks/CORE-001").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["id"], "CORE-001");
}

#[tokio::test]
async fn get_missing_task_returns_404() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, _) = get(&repo, "/api/tasks/CORE-999").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn start_transitions_task_and_persists() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = post(&repo, "/api/tasks/CORE-001/start", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["status"], "In Progress");
    let on_disk = fs::read_to_string(task_file(&repo, "CORE-001")).expect("task readable");
    assert!(on_disk.contains("status: In Progress"));
}

#[tokio::test]
async fn block_records_reason() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = post(
        &repo,
        "/api/tasks/CORE-001/block",
        Some(r#"{"reason":"Waiting on policy"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["status"], "Blocked");
    let on_disk = fs::read_to_string(task_file(&repo, "CORE-001")).expect("task readable");
    assert!(on_disk.contains("Waiting on policy"));
}

#[tokio::test]
async fn defer_records_reason() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = post(
        &repo,
        "/api/tasks/CORE-001/defer",
        Some(r#"{"reason":"Out of scope for now"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["status"], "Deferred");
    let on_disk = fs::read_to_string(task_file(&repo, "CORE-001")).expect("task readable");
    assert!(on_disk.contains("Out of scope for now"));
}

#[tokio::test]
async fn obsolete_transitions_task() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = post(
        &repo,
        "/api/tasks/CORE-001/obsolete",
        Some(r#"{"reason":"Superseded by CORE-002"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["status"], "Obsolete");
}

#[tokio::test]
async fn done_with_checked_criteria_succeeds() {
    let repo = task_repo();
    write_checked_task(&repo, "CORE-001", "Routing", "In Progress", "High");

    let (status, body) = post(&repo, "/api/tasks/CORE-001/done", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["status"], "Done");
}

#[tokio::test]
async fn block_without_reason_is_rejected() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, _) = post(&repo, "/api/tasks/CORE-001/block", Some("{}")).await;

    assert!(status.is_client_error(), "expected 4xx, got {status}");
}

#[tokio::test]
async fn done_with_unchecked_criteria_is_rejected() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "In Progress", "High");

    let (status, _) = post(&repo, "/api/tasks/CORE-001/done", None).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn invalid_repo_returns_422() {
    let repo = task_repo();
    // A status the schema does not allow makes the whole repo invalid.
    write_raw_task(&repo, "CORE-001", "status: Pending");

    let (status, _) = get(&repo, "/api/tasks").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

fn app(repo: &TempDir) -> Router {
    let state = AppState::single(repo.path().to_path_buf()).expect("state builds");
    router(state)
}

async fn get(repo: &TempDir, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds");
    send(repo, request).await
}

async fn post(repo: &TempDir, uri: &str, json: Option<&str>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method("POST").uri(uri);
    let body = match json {
        Some(payload) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Body::from(payload.to_string())
        }
        None => Body::empty(),
    };
    send(repo, builder.body(body).expect("request builds")).await
}

async fn send(repo: &TempDir, request: Request<Body>) -> (StatusCode, Value) {
    let response = app(repo).oneshot(request).await.expect("router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

fn task_repo() -> TempDir {
    let temp = tempfile::tempdir().expect("temp dir created");
    for domain in ["core", "app", "ecosystem"] {
        fs::create_dir_all(temp.path().join(".tasks").join(domain)).expect("domain dir created");
    }
    fs::write(temp.path().join(".tasks/task.schema.json"), "{}").expect("schema written");
    temp
}

fn task_file(repo: &TempDir, id: &str) -> std::path::PathBuf {
    repo.path()
        .join(".tasks/core")
        .join(format!("{id}-task.md"))
}

fn write_task(repo: &TempDir, id: &str, title: &str, status: &str, priority: &str) {
    fs::write(
        task_file(repo, id),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task written");
}

fn write_checked_task(repo: &TempDir, id: &str, title: &str, status: &str, priority: &str) {
    fs::write(
        task_file(repo, id),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: {priority}\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [x] Works\n"
        ),
    )
    .expect("task written");
}

fn write_raw_task(repo: &TempDir, id: &str, status_line: &str) {
    fs::write(
        task_file(repo, id),
        format!(
            "---\nid: {id}\ntitle: Routing\n{status_line}\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task written");
}
