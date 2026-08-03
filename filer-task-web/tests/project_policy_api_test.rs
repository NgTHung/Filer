use std::{fs, path::Path};

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
async fn registers_an_existing_project_and_serves_it_immediately() {
    let host = compatibility_project();
    let added = compatibility_project();
    write_compatibility_task(&added, "CORE-001", "Registered task");
    let app = app(&host).await;

    let (status, summary) = send(
        &app,
        json_request("POST", "/api/projects", json!({"path": added.path()})),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["name"], project_name(&added));
    assert_eq!(summary["task_count"], 1);
    assert_eq!(summary["domain_count"], 4);

    let (status, projects) = send(&app, get_request("/api/projects")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(projects.as_array().is_some_and(|projects| {
        projects
            .iter()
            .any(|project| project["name"] == project_name(&added))
    }));

    let (status, tasks) = send(&app, get_request(&project_uri(&added, "/tasks"))).await;
    assert_eq!(status, StatusCode::OK, "{tasks}");
    assert_eq!(tasks[0]["id"], "CORE-001");

    let (status, body) = send(
        &app,
        json_request("POST", "/api/projects", json!({"path": added.path()})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "duplicate_project_name");
    assert_eq!(body["project"], project_name(&added));
}

#[tokio::test]
async fn initializes_an_empty_directory_with_the_default_policy() {
    let host = compatibility_project();
    let added = tempfile::tempdir().expect("empty directory created");
    let app = app(&host).await;

    let (status, summary) = send(
        &app,
        json_request(
            "POST",
            "/api/projects",
            json!({"path": added.path(), "init": true}),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["name"], project_name(&added));
    assert_eq!(summary["task_count"], 0);
    assert_eq!(summary["domain_count"], 1);
    assert!(added.path().join(".tasks/config.json").is_file());

    let (status, policy) = send(&app, get_request(&project_uri(&added, "/policy"))).await;
    assert_eq!(status, StatusCode::OK, "{policy}");
    assert_eq!(
        policy,
        json!({
            "domains": {"default": {"prefixes": ["WORK"]}},
            "task_types": {"Feature": {"criteria": "acceptance"}},
            "tags": {"policy": "open"}
        })
    );
}

#[tokio::test]
async fn initializes_a_named_directory_created_under_the_given_path() {
    let host = compatibility_project();
    let parent = tempfile::tempdir().expect("parent directory created");
    let app = app(&host).await;

    let (status, summary) = send(
        &app,
        json_request(
            "POST",
            "/api/projects",
            json!({"path": parent.path(), "init": true, "name": "new-thing"}),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["name"], "new-thing");
    assert!(parent.path().join("new-thing/.tasks/config.json").is_file());
}

// A name whose directory is already there is the ordinary case of naming a
// project you have just made room for, so it initializes in place.
#[tokio::test]
async fn initializes_a_named_directory_that_already_exists() {
    let host = compatibility_project();
    let parent = tempfile::tempdir().expect("parent directory created");
    fs::create_dir(parent.path().join("existing")).expect("project directory created");
    let app = app(&host).await;

    let (status, summary) = send(
        &app,
        json_request(
            "POST",
            "/api/projects",
            json!({"path": parent.path(), "init": true, "name": "existing"}),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["name"], "existing");
}

// Naming a project that turns out to be there already is the find half of
// find-or-create: the caller wanted that project, so it opens instead of
// refusing.
#[tokio::test]
async fn opens_a_named_directory_that_already_holds_a_project() {
    let host = compatibility_project();
    let parent = tempfile::tempdir().expect("parent directory created");
    fs::create_dir_all(parent.path().join("existing/.tasks")).expect("project created");
    let app = app(&host).await;

    let (status, summary) = send(
        &app,
        json_request(
            "POST",
            "/api/projects",
            json!({"path": parent.path(), "init": true, "name": "existing"}),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(summary["name"], "existing");
}

#[tokio::test]
async fn rejects_a_name_that_is_empty_or_reaches_outside_the_given_path() {
    let host = compatibility_project();
    let parent = tempfile::tempdir().expect("parent directory created");
    let app = app(&host).await;

    for name in ["", "   ", "nested/deep", "nested\\deep", ".", ".."] {
        let (status, body) = send(
            &app,
            json_request(
                "POST",
                "/api/projects",
                json!({"path": parent.path(), "init": true, "name": name}),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{name:?}: {body}");
        assert_eq!(body["code"], "invalid_project_name");
        assert_eq!(body["field"], "name");
    }

    let entries = fs::read_dir(parent.path()).expect("parent directory read");
    assert_eq!(entries.count(), 0);
}

// Naming a project only means something while creating one, so a name sent
// without init is a mistake worth reporting rather than dropping.
#[tokio::test]
async fn rejects_a_name_sent_without_initialization() {
    let host = compatibility_project();
    let added = compatibility_project();
    let app = app(&host).await;

    let (status, body) = send(
        &app,
        json_request(
            "POST",
            "/api/projects",
            json!({"path": added.path(), "name": "renamed"}),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "name_requires_init");
    assert_eq!(body["field"], "name");
}

#[tokio::test]
async fn rejects_a_non_project_path_without_initialization() {
    let host = compatibility_project();
    let standalone = tempfile::tempdir().expect("standalone directory created");
    let app = app(&host).await;

    let (status, body) = send(
        &app,
        json_request("POST", "/api/projects", json!({"path": standalone.path()})),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "project_not_found");
    let (_, projects) = send(&app, get_request("/api/projects")).await;
    assert_eq!(projects.as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn policy_additions_are_visible_without_restarting() {
    let host = compatibility_project();
    let added = tempfile::tempdir().expect("empty directory created");
    let app = app(&host).await;
    let (status, _) = send(
        &app,
        json_request(
            "POST",
            "/api/projects",
            json!({"path": added.path(), "init": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let operations = [
        json!({"operation": "add_domain", "name": "backend", "prefixes": ["API"]}),
        json!({"operation": "add_prefix", "domain": "backend", "prefix": "WORK"}),
        json!({
            "operation": "add_task_type",
            "name": "ReleaseGate",
            "criteria": "exit",
            "role": "milestone"
        }),
        json!({"operation": "add_tag", "tag": "backend"}),
    ];

    for operation in operations {
        let (status, changed) = patch_policy(&app, &added, operation).await;
        assert_eq!(status, StatusCode::OK, "{changed}");
        let (status, fetched) = send(&app, get_request(&project_uri(&added, "/policy"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(changed, fetched);
    }

    let (_, policy) = send(&app, get_request(&project_uri(&added, "/policy"))).await;
    assert_eq!(
        policy["domains"]["backend"]["prefixes"],
        json!(["API", "WORK"])
    );
    assert_eq!(policy["task_types"]["ReleaseGate"]["criteria"], "exit");
    assert_eq!(policy["task_types"]["ReleaseGate"]["role"], "milestone");
    assert_eq!(
        policy["tags"],
        json!({"policy": "strict", "allowed": ["backend"]})
    );

    let (_, projects) = send(&app, get_request("/api/projects")).await;
    let summary = projects
        .as_array()
        .and_then(|projects| {
            projects
                .iter()
                .find(|project| project["name"] == project_name(&added))
        })
        .expect("registered project listed");
    assert_eq!(summary["domain_count"], 2);
    let (status, tasks) = send(&app, get_request(&project_uri(&added, "/tasks"))).await;
    assert_eq!(status, StatusCode::OK, "{tasks}");

    let (status, policy) = patch_policy(
        &app,
        &added,
        json!({"operation": "remove_domain", "name": "backend"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{policy}");
    assert!(policy["domains"].get("backend").is_none());
}

#[tokio::test]
async fn rejected_policy_removals_name_the_task_and_preserve_config_bytes() {
    let host = compatibility_project();
    let added = configured_project_with_task();
    let app = app(&host).await;
    let (status, _) = send(
        &app,
        json_request("POST", "/api/projects", json!({"path": added.path()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let removals = [
        json!({"operation": "remove_prefix", "domain": "core", "prefix": "CORE"}),
        json!({"operation": "remove_task_type", "name": "Feature"}),
        json!({"operation": "remove_tag", "tag": "tasks"}),
    ];
    let config_path = added.path().join(".tasks/config.json");

    for operation in removals {
        let before = fs::read(&config_path).expect("config read before rejection");
        let (status, body) = patch_policy(&app, &added, operation).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["code"], "validation_failed");
        assert!(body["issues"].as_array().is_some_and(|issues| {
            issues
                .iter()
                .any(|issue| issue["context"]["task"] == "core:CORE-001")
        }));
        assert_eq!(
            fs::read(&config_path).expect("config read after rejection"),
            before
        );
    }
}

async fn app(repo: &TempDir) -> Router {
    let storage = Storage::open(repo.path().join("project-policy-api-test.sqlite3"))
        .await
        .expect("storage opens");
    common::authenticated_router(
        AppState::single(repo.path().to_path_buf(), storage).expect("state builds"),
    )
    .await
}

async fn patch_policy(app: &Router, repo: &TempDir, operation: Value) -> (StatusCode, Value) {
    send(
        app,
        json_request("PATCH", &project_uri(repo, "/policy"), operation),
    )
    .await
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds")
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

fn compatibility_project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
    repo
}

fn configured_project_with_task() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("domain created");
    fs::write(
        repo.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {"core": {"prefixes": ["CORE", "ALT"]}},
  "task_types": {
    "Feature": {"criteria": "acceptance"},
    "Bug": {"criteria": "acceptance"}
  },
  "tags": {"policy": "strict", "allowed": ["tasks", "spare"]}
}
"#,
    )
    .expect("config written");
    write_task(
        repo.path(),
        "CORE-001",
        "Configured task",
        "tags: [tasks]\n",
    );
    repo
}

fn write_compatibility_task(repo: &TempDir, id: &str, title: &str) {
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("domain created");
    write_task(repo.path(), id, title, "");
}

fn write_task(root: &Path, id: &str, title: &str, extra: &str) {
    fs::write(
        root.join(".tasks/core").join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n{extra}---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
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
