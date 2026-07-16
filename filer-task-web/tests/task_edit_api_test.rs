use std::{fs, path::PathBuf};

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
async fn patch_applies_all_fields_and_returns_the_refreshed_show_view() {
    let repo = project();
    write_milestone(&repo, "MILESTONE-001", "0.3.0");
    write_task(&repo, "CORE-001", "Parent task", "", "Parent summary.");
    write_task(
        &repo,
        "CORE-002",
        "Dependency task",
        "",
        "Dependency summary.",
    );
    write_task(
        &repo,
        "CORE-003",
        "Editable task",
        "risk: Low\nimpact: Original impact text.\ntags: [old]\n",
        "Original summary.\n\n## Implementation\n\nOriginal body text.",
    );

    let patch = json!({
        "title": "Updated task title",
        "summary": "This value is replaced by sections.Summary.",
        "sections": {
            "Summary": "Updated summary.",
            "Implementation": "Updated body text."
        },
        "risk": "High",
        "impact": "Touches task editing.",
        "tags": ["tasks", "web"],
        "milestone": "0.3.0",
        "parent": "core:CORE-001",
        "depends_on": ["core:CORE-002"]
    });
    let (status, body) = send(&repo, edit_request(&repo, "CORE-003", patch)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["schema_version"], 2);
    assert_eq!(body["detail"]["task"]["qualified_id"], "core:CORE-003");
    assert_eq!(body["detail"]["task"]["title"], "Updated task title");
    assert_eq!(body["detail"]["task"]["risk"], "High");
    assert_eq!(body["detail"]["task"]["impact"], "Touches task editing.");
    assert_eq!(body["detail"]["task"]["tags"], json!(["tasks", "web"]));
    assert_eq!(body["detail"]["task"]["milestone"], "0.3.0");
    assert_eq!(body["detail"]["task"]["parent"], "core:CORE-001");
    assert_eq!(
        body["detail"]["task"]["depends_on"],
        json!(["core:CORE-002"])
    );
    assert_section(&body, "Summary", "Updated summary.");
    assert_section(&body, "Implementation", "Updated body text.");

    let written = fs::read_to_string(find_task(&repo, "CORE-003")).expect("task read");
    assert!(!written.contains("This value is replaced"));
}

#[tokio::test]
async fn patch_keeps_omitted_fields_and_clears_explicit_nullable_and_list_fields() {
    let repo = project();
    write_milestone(&repo, "MILESTONE-001", "0.3.0");
    write_task(&repo, "CORE-001", "Parent task", "", "Parent summary.");
    write_task(
        &repo,
        "CORE-002",
        "Dependency task",
        "",
        "Dependency summary.",
    );
    write_task(
        &repo,
        "CORE-003",
        "Editable task",
        concat!(
            "parent: core:CORE-001\n",
            "milestone: 0.3.0\n",
            "depends_on: [core:CORE-002]\n",
            "risk: High\n",
            "impact: Original impact text.\n",
            "tags: [tasks, web]\n"
        ),
        "Original summary.\n\n## Implementation\n\nOriginal body text.",
    );

    let patch = json!({
        "risk": null,
        "impact": null,
        "tags": [],
        "milestone": null,
        "parent": null,
        "depends_on": []
    });
    let (status, body) = send(&repo, edit_request(&repo, "core:CORE-003", patch)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["detail"]["task"]["title"], "Editable task");
    for field in [
        "risk",
        "impact",
        "tags",
        "milestone",
        "parent",
        "depends_on",
    ] {
        assert!(
            body["detail"]["task"].get(field).is_none(),
            "{field} was not cleared: {body}"
        );
    }
    assert_section(&body, "Summary", "Original summary.");
    assert_section(&body, "Implementation", "Original body text.");
}

#[tokio::test]
async fn non_nullable_patch_fields_reject_null_without_writing() {
    for field in ["title", "summary", "sections", "tags", "depends_on"] {
        let repo = project();
        write_task(
            &repo,
            "CORE-001",
            "Editable task",
            "tags: [tasks]\n",
            "Original summary.",
        );
        let path = find_task(&repo, "CORE-001");
        let before = fs::read(&path).expect("task read");

        let (status, _) = send(
            &repo,
            edit_request(&repo, "core:CORE-001", json!({field: null})),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "field {field}");
        assert_eq!(fs::read(path).expect("task read"), before, "field {field}");
    }
}

#[tokio::test]
async fn dependency_cycle_promotes_the_original_issue_and_does_not_write() {
    let repo = project();
    write_task(
        &repo,
        "CORE-001",
        "First task",
        "depends_on: [CORE-002]\n",
        "First summary.",
    );
    write_task(&repo, "CORE-002", "Second task", "", "Second summary.");
    let path = find_task(&repo, "CORE-002");
    let before = fs::read(&path).expect("task read");

    let (status, body) = send(
        &repo,
        edit_request(&repo, "core:CORE-002", json!({"depends_on": ["CORE-001"]})),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_error");
    assert_eq!(body["field"], "depends_on");
    assert_eq!(body["issues"][0]["code"], "validation_error");
    assert_eq!(body["context"], body["issues"][0]["context"]);
    assert!(
        body["issues"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("dependency cycle detected"))
    );
    assert_eq!(fs::read(path).expect("task read"), before);
}

#[tokio::test]
async fn unknown_milestone_promotes_the_original_issue_and_does_not_write() {
    let repo = project();
    write_milestone(&repo, "MILESTONE-001", "0.3.0");
    write_task(&repo, "CORE-001", "Editable task", "", "Original summary.");
    let path = find_task(&repo, "CORE-001");
    let before = fs::read(&path).expect("task read");

    let (status, body) = send(
        &repo,
        edit_request(&repo, "core:CORE-001", json!({"milestone": "9.9.9"})),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_error");
    assert_eq!(body["field"], "milestone");
    assert_eq!(body["issues"][0]["code"], "validation_error");
    assert_eq!(body["context"], body["issues"][0]["context"]);
    assert!(
        body["issues"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("milestone 9.9.9"))
    );
    assert_eq!(fs::read(path).expect("task read"), before);
}

#[tokio::test]
async fn strict_tag_rejection_promotes_policy_context_and_does_not_write() {
    let repo = strict_project();
    write_task(
        &repo,
        "CORE-001",
        "Editable task",
        "tags: [tasks]\n",
        "Original summary.",
    );
    let path = find_task(&repo, "CORE-001");
    let before = fs::read(&path).expect("task read");

    let (status, body) = send(
        &repo,
        edit_request(&repo, "core:CORE-001", json!({"tags": ["rejected"]})),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "tag_rejected");
    assert_eq!(body["field"], "tags");
    assert_eq!(body["context"]["rejected_value"], "rejected");
    assert_eq!(body["context"]["policy"], "strict");
    assert_eq!(body["context"]["allowed"], json!(["tasks"]));
    assert_eq!(body["issues"][0]["code"], "tag_rejected");
    assert_eq!(body["issues"][0]["context"], body["context"]);
    assert_eq!(fs::read(path).expect("task read"), before);
}

#[tokio::test]
async fn edit_route_rejects_unknown_targets_and_has_no_unscoped_form() {
    let repo = project();
    write_task(&repo, "CORE-001", "Editable task", "", "Original summary.");
    let app = app(&repo).await;

    let unknown_project = Request::builder()
        .method("PATCH")
        .uri("/api/projects/ghost/tasks/core:CORE-001")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"title":"Updated task"}"#))
        .expect("request builds");
    assert_eq!(
        send_with(&app, unknown_project).await.0,
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        send_with(
            &app,
            edit_request(&repo, "core:CORE-999", json!({"title": "Updated task"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let unscoped = Request::builder()
        .method("PATCH")
        .uri("/api/tasks/core:CORE-001")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"title":"Updated task"}"#))
        .expect("request builds");
    assert_eq!(send_with(&app, unscoped).await.0, StatusCode::NOT_FOUND);
}

fn edit_request(repo: &TempDir, identity: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(project_uri(repo, &format!("/tasks/{identity}")))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

async fn send(repo: &TempDir, request: Request<Body>) -> (StatusCode, Value) {
    send_with(&app(repo).await, request).await
}

async fn send_with(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn app(repo: &TempDir) -> Router {
    let storage = Storage::open(repo.path().join("task-edit-api-test.sqlite3"))
        .await
        .expect("storage opens");
    let state = AppState::single(repo.path().to_path_buf(), storage).expect("state builds");
    common::authenticated_router(state).await
}

fn project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    for domain in ["core", "app", "ecosystem", "milestones"] {
        fs::create_dir_all(repo.path().join(".tasks").join(domain)).expect("domain created");
    }
    repo
}

fn strict_project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("domain created");
    fs::write(
        repo.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {"core": {"prefixes": ["CORE"]}},
  "task_types": {"Feature": {"criteria": "acceptance"}},
  "tags": {"policy": "strict", "allowed": ["tasks"]}
}"#,
    )
    .expect("configuration written");
    repo
}

fn write_task(repo: &TempDir, id: &str, title: &str, metadata: &str, body: &str) {
    fs::write(
        repo.path().join(".tasks/core").join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n{metadata}---\n\n## Summary\n\n{body}\n\n## Acceptance Criteria\n\n- [ ] Works\n"
        ),
    )
    .expect("task written");
}

fn write_milestone(repo: &TempDir, id: &str, milestone: &str) {
    fs::write(
        repo.path()
            .join(".tasks/milestones")
            .join(format!("{id}-milestone.md")),
        format!(
            "---\nid: {id}\ntitle: Project milestone\nstatus: To Do\npriority: High\ntype: Milestone\nmilestone: {milestone}\n---\n\n## Exit Criteria\n\n- [ ] Ships\n"
        ),
    )
    .expect("milestone written");
}

fn find_task(repo: &TempDir, id: &str) -> PathBuf {
    fs::read_dir(repo.path().join(".tasks/core"))
        .expect("domain readable")
        .map(|entry| entry.expect("entry readable").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&format!("{id}-")))
        })
        .expect("task exists")
}

fn project_uri(repo: &TempDir, suffix: &str) -> String {
    let name = repo
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name");
    format!("/api/projects/{name}{suffix}")
}

fn assert_section(body: &Value, heading: &str, content: &str) {
    let section = body["detail"]["sections"]
        .as_array()
        .and_then(|sections| {
            sections
                .iter()
                .find(|section| section["heading"] == heading)
        })
        .unwrap_or_else(|| panic!("missing section {heading}: {body}"));
    assert_eq!(section["content"], content);
}
