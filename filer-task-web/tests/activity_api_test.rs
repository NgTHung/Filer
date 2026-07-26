use std::fs;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::{app::AppState, storage::Storage};

mod common;

#[tokio::test]
async fn successful_task_creation_records_one_activity_row() {
    let repo = project();
    let app = app(&repo).await;
    let username = identity_username(&app).await;

    let (status, _) = send(&app, create_request(&repo, "CORE", "001")).await;
    assert_eq!(status, StatusCode::OK);

    let rows = activity_for_project(&app, &project_name(&repo)).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], "task.create");
    assert_eq!(rows[0]["username"], username);
    assert_eq!(rows[0]["task_id"], "core:CORE-001");
    assert_eq!(rows[0]["project"], project_name(&repo));
}

#[tokio::test]
async fn failed_task_creation_records_no_activity_row() {
    let repo = project();
    write_task(&repo, "CORE-001", "Existing task", "To Do", "- [ ] Works\n");
    let app = app(&repo).await;

    let (status, _) = send(&app, create_request(&repo, "CORE", "001")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let rows = activity(&app, "/api/activity").await;
    assert!(rows.is_empty());
}

#[tokio::test]
async fn field_edit_records_one_activity_row() {
    let repo = project();
    write_task(
        &repo,
        "CORE-001",
        "Original title",
        "To Do",
        "- [ ] Works\n",
    );
    let app = app(&repo).await;

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &project_uri(&repo, "/tasks/core:CORE-001"),
            json!({"title": "Updated title"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let rows = activity(&app, "/api/activity").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], "task.edit");
    assert_eq!(rows[0]["task_id"], "core:CORE-001");
}

#[tokio::test]
async fn criterion_toggle_records_one_activity_row_with_detail() {
    let repo = project();
    write_task(&repo, "CORE-001", "Criteria task", "To Do", "- [ ] Works\n");
    let app = app(&repo).await;
    let (_, shown) = send(
        &app,
        get_request(&project_uri(&repo, "/tasks/core:CORE-001")),
    )
    .await;
    let hash = shown["detail"]["criteria"][0]["content_hash"]
        .as_str()
        .expect("hash returned")
        .to_string();

    let request = Request::builder()
        .method("PUT")
        .uri(project_uri(&repo, "/tasks/core:CORE-001/criteria/0"))
        .header(CONTENT_TYPE, "application/json")
        .header("if-match", format!("\"{hash}\""))
        .body(Body::from(json!({"checked": true}).to_string()))
        .expect("request builds");
    let (status, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);

    let rows = activity(&app, "/api/activity").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], "task.criterion");
    assert_eq!(rows[0]["detail"], "index 0 = true");
}

#[tokio::test]
async fn each_lifecycle_transition_records_its_own_action() {
    let repo = project();
    write_task(&repo, "CORE-001", "Start me", "To Do", "- [ ] Works\n");
    write_task(&repo, "CORE-002", "Block me", "To Do", "- [ ] Works\n");
    write_task(&repo, "CORE-003", "Defer me", "To Do", "- [ ] Works\n");
    write_task(
        &repo,
        "CORE-004",
        "Make me obsolete",
        "To Do",
        "- [ ] Works\n",
    );
    write_task(
        &repo,
        "CORE-005",
        "Finish me",
        "In Progress",
        "- [x] Works\n",
    );
    let app = app(&repo).await;

    let (status, _) = send(
        &app,
        post_request(&project_uri(&repo, "/tasks/core:CORE-001/start"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        post_request(
            &project_uri(&repo, "/tasks/core:CORE-002/block"),
            Some(json!({"reason": "waiting on review"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        post_request(
            &project_uri(&repo, "/tasks/core:CORE-003/defer"),
            Some(json!({"reason": "out of scope"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        post_request(
            &project_uri(&repo, "/tasks/core:CORE-004/obsolete"),
            Some(json!({"reason": "superseded"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        post_request(&project_uri(&repo, "/tasks/core:CORE-005/done"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let rows = activity(&app, "/api/activity?limit=50").await;
    assert_eq!(rows.len(), 5);
    let actions: Vec<&str> = rows
        .iter()
        .rev()
        .map(|row| row["action"].as_str().expect("action is a string"))
        .collect();
    assert_eq!(
        actions,
        vec![
            "task.start",
            "task.block",
            "task.defer",
            "task.obsolete",
            "task.done",
        ]
    );
    let block_row = rows
        .iter()
        .find(|row| row["action"] == "task.block")
        .expect("block row recorded");
    assert_eq!(block_row["detail"], "waiting on review");
}

#[tokio::test]
async fn project_register_and_deregister_record_activity_without_a_task_id() {
    let host = project();
    let added = project();
    write_task(
        &added,
        "CORE-001",
        "Registered task",
        "To Do",
        "- [ ] Works\n",
    );
    let app = app(&host).await;

    let (status, _) = send(
        &app,
        json_request("POST", "/api/projects", json!({"path": added.path()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let name = project_name(&added);
    let (status, _) = send(
        &app,
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/projects/{name}"))
            .body(Body::empty())
            .expect("request builds"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let rows = activity(&app, &format!("/api/activity?project={name}")).await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row["task_id"].is_null()));
    let actions: Vec<&str> = rows
        .iter()
        .map(|row| row["action"].as_str().expect("action is a string"))
        .collect();
    assert_eq!(actions, vec!["project.deregister", "project.register"]);
}

#[tokio::test]
async fn policy_mutation_records_activity_without_a_task_id() {
    let repo = project();
    let app = app(&repo).await;

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &project_uri(&repo, "/policy"),
            json!({"operation": "add_tag", "tag": "backend"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let rows = activity(&app, "/api/activity").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], "policy.add_tag");
    assert_eq!(rows[0]["detail"], "backend");
    assert!(rows[0]["task_id"].is_null());
}

#[tokio::test]
async fn policy_activity_detail_names_the_changed_operand() {
    let repo = project();
    let app = app(&repo).await;

    let (status, _) = send(
        &app,
        json_request(
            "PATCH",
            &project_uri(&repo, "/policy"),
            json!({"operation": "add_domain", "name": "infra", "prefixes": ["INFRA"]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let rows = activity(&app, "/api/activity").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], "policy.add_domain");
    assert_eq!(rows[0]["detail"], "infra [INFRA]");
}

#[tokio::test]
async fn activity_list_paginates_newest_first() {
    let repo = project();
    let app = app(&repo).await;
    for number in ["001", "002", "003"] {
        let (status, _) = send(&app, create_request(&repo, "CORE", number)).await;
        assert_eq!(status, StatusCode::OK);
    }

    let page = activity(&app, "/api/activity?limit=2&offset=1").await;
    let task_ids: Vec<&str> = page
        .iter()
        .map(|row| row["task_id"].as_str().expect("task id is a string"))
        .collect();
    assert_eq!(task_ids, vec!["core:CORE-002", "core:CORE-001"]);
}

fn create_request(repo: &TempDir, prefix: &str, number: &str) -> Request<Body> {
    json_request(
        "POST",
        &project_uri(repo, "/tasks"),
        json!({
            "domain": "core",
            "prefix": prefix,
            "number": number,
            "title": "Created task",
            "type": "Feature",
            "priority": "High",
            "milestone": null,
            "tags": []
        }),
    )
}

fn post_request(uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri);
    let payload = match body {
        Some(body) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    builder.body(payload).expect("request builds")
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds")
}

async fn activity(app: &Router, uri: &str) -> Vec<Value> {
    let (status, body) = send(app, get_request(uri)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array().expect("activity is an array").clone()
}

async fn activity_for_project(app: &Router, project: &str) -> Vec<Value> {
    activity(app, &format!("/api/activity?project={project}")).await
}

async fn identity_username(app: &Router) -> String {
    let (status, body) = send(app, get_request("/api/identity")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["username"]
        .as_str()
        .expect("identity returns a username")
        .to_string()
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("router responds");
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

async fn app(repo: &TempDir) -> Router {
    let storage = Storage::open(repo.path().join("activity-api-test.sqlite3"))
        .await
        .expect("storage opens");
    let state = AppState::single(repo.path().to_path_buf(), storage).expect("state builds");
    common::authenticated_router(state).await
}

fn project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    for domain in ["core", "app", "ecosystem"] {
        fs::create_dir_all(repo.path().join(".tasks").join(domain)).expect("domain created");
    }
    repo
}

fn write_task(repo: &TempDir, id: &str, title: &str, status: &str, criteria: &str) {
    let path = repo
        .path()
        .join(".tasks/core")
        .join(format!("{id}-task.md"));
    fs::write(
        path,
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n{criteria}"
        ),
    )
    .expect("task written");
}

fn project_name(repo: &TempDir) -> String {
    repo.path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name")
        .to_string()
}

fn project_uri(repo: &TempDir, suffix: &str) -> String {
    format!("/api/projects/{}{suffix}", project_name(repo))
}
