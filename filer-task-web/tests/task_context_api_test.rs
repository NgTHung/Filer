use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::{app::AppState, storage::Storage};

mod common;

#[tokio::test]
async fn context_returns_relations_and_the_root_first_ancestor_chain() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Root task", "In Progress", "");
    write_task(
        &repo,
        "CORE-002",
        "Middle task",
        "In Progress",
        "parent: CORE-001\n",
    );
    write_task(
        &repo,
        "CORE-003",
        "Target task",
        "To Do",
        "parent: CORE-002\n",
    );
    write_task(
        &repo,
        "CORE-004",
        "Sibling task",
        "To Do",
        "parent: CORE-002\n",
    );
    write_task(&repo, "CORE-005", "Dependency task", "In Progress", "");
    write_task(
        &repo,
        "CORE-006",
        "Dependent task",
        "To Do",
        "depends_on: [CORE-003]\n",
    );
    write_task(
        &repo,
        "CORE-007",
        "Child task",
        "To Do",
        "parent: CORE-003\ndepends_on: [CORE-005]\n",
    );

    let (status, body) = get(&repo, "/tasks/core:CORE-003/context").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["qualified_id"], "core:CORE-003");
    assert_eq!(
        qualified_ids(&body["ancestors"]),
        ["core:CORE-001", "core:CORE-002"]
    );
    assert_eq!(body["parent"]["task"]["qualified_id"], "core:CORE-002");
    assert_eq!(related_ids(&body["children"]), ["core:CORE-007"]);
    assert_eq!(related_ids(&body["dependents"]), ["core:CORE-006"]);

    let (status, child) = get(&repo, "/tasks/core:CORE-007/context").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(related_ids(&child["dependencies"]), ["core:CORE-005"]);
    assert_eq!(
        qualified_ids(&child["ancestors"]),
        ["core:CORE-001", "core:CORE-002", "core:CORE-003"]
    );
}

#[tokio::test]
async fn context_reports_readiness_blockers_for_an_unfinished_dependency() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Dependency task", "In Progress", "");
    write_task(
        &repo,
        "CORE-002",
        "Target task",
        "To Do",
        "depends_on: [CORE-001]\n",
    );

    let (status, body) = get(&repo, "/tasks/core:CORE-002/context").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["readiness"]["ready"], false);
    assert_eq!(body["readiness"]["blockers"][0]["kind"], "dependency");
    assert_eq!(body["readiness"]["blockers"][0]["task_id"], "core:CORE-001");
}

#[tokio::test]
async fn context_returns_an_empty_chain_for_a_root_task() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Root task", "To Do", "");

    let (status, body) = get(&repo, "/tasks/core:CORE-001/context").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body["ancestors"]
            .as_array()
            .expect("ancestors array")
            .is_empty()
    );
    assert!(body["parent"].is_null());
}

#[tokio::test]
async fn context_rejects_an_unknown_task() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Root task", "To Do", "");

    let (status, _) = get(&repo, "/tasks/core:CORE-999/context").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn qualified_ids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("task array")
        .iter()
        .map(|task| {
            task["qualified_id"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

fn related_ids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("related array")
        .iter()
        .map(|related| {
            related["task"]["qualified_id"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

async fn get(repo: &TempDir, suffix: &str) -> (StatusCode, Value) {
    let uri = format!("/api/projects/{}{suffix}", project_name(repo));
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds");
    let response = app(repo)
        .await
        .oneshot(request)
        .await
        .expect("router responds");
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
    let storage = Storage::open(repo.path().join("context-test.sqlite3"))
        .await
        .expect("test storage opens and migrates");
    let state = AppState::single(repo.path().to_path_buf(), storage).expect("state builds");
    common::authenticated_router(state).await
}

fn project_name(repo: &TempDir) -> String {
    repo.path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name")
        .to_string()
}

fn task_repo() -> TempDir {
    let temp = tempfile::tempdir().expect("temp dir created");
    for domain in ["core", "app", "ecosystem"] {
        std::fs::create_dir_all(temp.path().join(".tasks").join(domain))
            .expect("domain dir created");
    }
    temp
}

fn write_task(repo: &TempDir, id: &str, title: &str, status: &str, extra: &str) {
    std::fs::write(
        repo.path()
            .join(".tasks/core")
            .join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: {status}\npriority: High\ntype: Feature\n{extra}---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task written");
}
