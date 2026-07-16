use std::{fs, path::PathBuf};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use filer_task::project::TaskProject;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::{
    app::{AppState, router},
    error::WebError,
    registry::ProjectRegistry,
    storage::Storage,
};

#[tokio::test]
async fn list_returns_seeded_tasks() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");
    write_task(&repo, "CORE-002", "Caching", "In Progress", "Low");

    let (status, body) = get(&repo, "/tasks").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("array").len(), 2);
}

#[tokio::test]
async fn list_filters_by_status() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");
    write_task(&repo, "CORE-002", "Caching", "In Progress", "Low");

    let (status, body) = get(&repo, "/tasks?status=In%20Progress").await;

    assert_eq!(status, StatusCode::OK);
    let tasks = body.as_array().expect("array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "CORE-002");
}

#[tokio::test]
async fn policy_returns_config_shaped_effective_policy() {
    let repo = configured_repo(
        r#"{
          "version": 1,
          "domains": {"backend": {"prefixes": ["API"]}},
          "task_types": {
            "Feature": {"criteria": "acceptance"},
            "ReleaseGate": {"criteria": "exit", "role": "milestone"}
          },
          "tags": {"policy": "strict", "allowed": ["backend"]}
        }"#,
        &["backend"],
    );

    let (status, body) = get(&repo, "/policy").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "domains": {
                "backend": {"prefixes": ["API"]}
            },
            "task_types": {
                "Feature": {"criteria": "acceptance"},
                "ReleaseGate": {"criteria": "exit", "role": "milestone"}
            },
            "tags": {
                "policy": "strict",
                "allowed": ["backend"]
            }
        })
    );
}

#[tokio::test]
async fn policy_returns_compatibility_defaults_without_config() {
    let repo = task_repo();

    let (status, body) = get(&repo, "/policy").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("version").is_none());
    assert_eq!(body["domains"].as_object().expect("domains").len(), 4);
    assert_eq!(
        body["domains"]["milestones"]["prefixes"],
        json!(["MILESTONE"])
    );
    assert!(
        body["domains"]["core"]["prefixes"]
            .as_array()
            .expect("core prefixes")
            .contains(&json!("CORE"))
    );
    assert_eq!(body["task_types"]["Feature"]["criteria"], "acceptance");
    assert_eq!(body["task_types"]["Epic"]["criteria"], "exit");
    assert_eq!(body["task_types"]["Milestone"]["role"], "milestone");
    assert_eq!(body["tags"], json!({"policy": "open"}));
}

#[tokio::test]
async fn policy_does_not_require_valid_task_files() {
    let repo = task_repo();
    write_raw_task(&repo, "CORE-001", "status: Pending");

    let (status, body) = get(&repo, "/policy").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tags"], json!({"policy": "open"}));
}

#[tokio::test]
async fn strict_policy_rejects_unknown_tag_filter_with_catalog_context() {
    let repo = configured_repo(
        r#"{
          "version": 1,
          "domains": {"backend": {"prefixes": ["WORK"]}},
          "task_types": {"Feature": {"criteria": "acceptance"}},
          "tags": {"policy": "strict", "allowed": ["backend", "tasks"]}
        }"#,
        &["backend"],
    );

    let (status, body) = get(&repo, "/tasks?tag=frontend").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "tag_rejected");
    assert_eq!(body["context"]["rejected_value"], "frontend");
    assert_eq!(body["context"]["allowed"], json!(["backend", "tasks"]));
}

#[tokio::test]
async fn open_policy_applies_unknown_tag_filter() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = get(&repo, "/tasks?tag=frontend").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn ambiguous_bare_parent_filter_returns_sorted_candidates() {
    let repo = duplicate_id_repo();
    write_domain_task(&repo, "backend", "WORK-001", "Backend parent", "");
    write_domain_task(&repo, "release", "WORK-001", "Release parent", "");

    let (status, body) = get(&repo, "/tasks?parent=WORK-001").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "ambiguous_reference");
    assert_eq!(
        body["context"]["candidates"],
        json!(["backend:WORK-001", "release:WORK-001"])
    );
}

#[tokio::test]
async fn unique_bare_parent_filter_resolves_project_wide() {
    let repo = duplicate_id_repo();
    write_domain_task(&repo, "backend", "WORK-001", "Backend parent", "");
    write_domain_task(
        &repo,
        "backend",
        "WORK-002",
        "Backend child",
        "parent: WORK-001\n",
    );

    let (status, body) = get(&repo, "/tasks?parent=WORK-001").await;

    assert_eq!(status, StatusCode::OK);
    let tasks = body.as_array().expect("task array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["qualified_id"], "backend:WORK-002");
}

#[tokio::test]
async fn qualified_parent_filter_still_targets_exact_identity() {
    let repo = duplicate_id_repo();
    write_domain_task(&repo, "backend", "WORK-001", "Backend parent", "");
    write_domain_task(&repo, "release", "WORK-001", "Release parent", "");
    write_domain_task(
        &repo,
        "backend",
        "WORK-002",
        "Backend child",
        "parent: WORK-001\n",
    );

    let (status, body) = get(&repo, "/tasks?parent=backend:WORK-001").await;

    assert_eq!(status, StatusCode::OK);
    let tasks = body.as_array().expect("task array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["qualified_id"], "backend:WORK-002");
}

#[tokio::test]
async fn missing_parent_filter_returns_task_not_found() {
    let repo = duplicate_id_repo();

    let (status, body) = get(&repo, "/tasks?parent=WORK-999").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "task_not_found");
    assert_eq!(body["context"]["reference"], "WORK-999");
}

#[tokio::test]
async fn get_task_returns_detail() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = get(&repo, "/tasks/core:CORE-001").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["id"], "CORE-001");
}

#[tokio::test]
async fn get_missing_task_returns_404() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, _) = get(&repo, "/tasks/core:CORE-999").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn start_transitions_task_and_persists() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, body) = post(&repo, "/tasks/core:CORE-001/start", None).await;

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
        "/tasks/core:CORE-001/block",
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
        "/tasks/core:CORE-001/defer",
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
        "/tasks/core:CORE-001/obsolete",
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

    let (status, body) = post(&repo, "/tasks/core:CORE-001/done", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["status"], "Done");
}

#[tokio::test]
async fn block_without_reason_is_rejected() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "To Do", "High");

    let (status, _) = post(&repo, "/tasks/core:CORE-001/block", Some("{}")).await;

    assert!(status.is_client_error(), "expected 4xx, got {status}");
}

#[tokio::test]
async fn done_with_unchecked_criteria_is_rejected() {
    let repo = task_repo();
    write_task(&repo, "CORE-001", "Routing", "In Progress", "High");

    let (status, _) = post(&repo, "/tasks/core:CORE-001/done", None).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn invalid_repo_returns_422() {
    let repo = task_repo();
    // A status the schema does not allow makes the whole repo invalid.
    write_raw_task(&repo, "CORE-001", "status: Pending");

    let (status, _) = get(&repo, "/tasks").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn projects_list_valid_and_broken_repositories() {
    let valid = task_repo();
    let broken = task_repo();
    write_task(&valid, "CORE-001", "Routing", "To Do", "High");
    write_raw_task(&broken, "CORE-002", "status: Pending");
    let roots = vec![valid.path().to_path_buf(), broken.path().to_path_buf()];

    let (status, body) = send_roots(&roots, request("/api/projects")).await;

    assert_eq!(status, StatusCode::OK);
    let projects = body.as_array().expect("projects array");
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0]["name"], project_name(&valid));
    assert_eq!(projects[0]["task_count"], 1);
    assert_eq!(projects[0]["domain_count"], domain_count(&valid));
    assert_eq!(projects[0]["broken"], false);
    assert!(projects[0].get("issues").is_none());
    assert_eq!(projects[1]["name"], project_name(&broken));
    assert_eq!(projects[1]["broken"], true);
    let issue = &projects[1]["issues"][0];
    assert!(issue.get("code").is_some());
    assert!(issue.get("path").is_some());
    assert!(issue.get("message").is_some());
    assert!(issue.get("context").is_some());
}

#[tokio::test]
async fn broken_project_does_not_disable_another_project() {
    let valid = task_repo();
    let broken = task_repo();
    write_task(&valid, "CORE-001", "Routing", "To Do", "High");
    write_raw_task(&broken, "CORE-002", "status: Pending");
    let roots = vec![valid.path().to_path_buf(), broken.path().to_path_buf()];
    let app = app_from_roots(&roots).await;

    let valid_uri = project_endpoint(&valid, "/tasks");
    let valid_response = app
        .clone()
        .oneshot(request(&valid_uri))
        .await
        .expect("router responds");
    let (valid_status, valid_body) = response_value(valid_response).await;
    let broken_uri = project_endpoint(&broken, "/tasks");
    let broken_response = app
        .oneshot(request(&broken_uri))
        .await
        .expect("router responds");
    let (broken_status, broken_body) = response_value(broken_response).await;

    assert_eq!(valid_status, StatusCode::OK);
    assert_eq!(valid_body.as_array().expect("tasks array").len(), 1);
    assert_eq!(broken_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(broken_body["project"], project_name(&broken));
    assert!(broken_body["issues"].is_array());
}

#[tokio::test]
async fn broken_project_transition_fails_before_writing() {
    let broken = task_repo();
    write_raw_task(&broken, "CORE-002", "status: Pending");
    let path = task_file(&broken, "CORE-002");
    let before = fs::read_to_string(&path).expect("task readable");
    let uri = project_endpoint(&broken, "/tasks/core:CORE-002/start");
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .expect("request builds");

    let (status, body) = send_roots(&[broken.path().to_path_buf()], request).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["project"], project_name(&broken));
    assert_eq!(fs::read_to_string(path).expect("task readable"), before);
}

#[tokio::test]
async fn unknown_project_returns_not_found() {
    let repo = task_repo();

    let (status, _) = send(&repo, request("/api/projects/ghost/tasks")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn project_prefixed_transition_only_changes_selected_project() {
    let selected = task_repo();
    let untouched = task_repo();
    write_task(&selected, "CORE-001", "Routing", "To Do", "High");
    write_task(&untouched, "CORE-001", "Routing", "To Do", "High");
    let roots = vec![
        selected.path().to_path_buf(),
        untouched.path().to_path_buf(),
    ];
    let uri = project_endpoint(&selected, "/tasks/core:CORE-001/start");
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .expect("request builds");

    let (status, _) = send_roots(&roots, request).await;

    assert_eq!(status, StatusCode::OK);
    let selected_task = fs::read_to_string(task_file(&selected, "CORE-001")).expect("task");
    let untouched_task = fs::read_to_string(task_file(&untouched, "CORE-001")).expect("task");
    assert!(selected_task.contains("status: In Progress"));
    assert!(untouched_task.contains("status: To Do"));
}

#[tokio::test]
async fn unscoped_task_routes_are_removed() {
    let repo = task_repo();

    let (status, _) = send(&repo, request("/api/tasks")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn registry_rejects_duplicate_directory_names() {
    let first_parent = tempfile::tempdir().expect("temp dir created");
    let second_parent = tempfile::tempdir().expect("temp dir created");
    let first = first_parent.path().join("shared");
    let second = second_parent.path().join("shared");
    create_task_repo(&first);
    create_task_repo(&second);

    let error = ProjectRegistry::from_roots(vec![first, second]);

    assert!(matches!(error, Err(WebError::DuplicateProjectName(name)) if name == "shared"));
}

#[test]
fn registry_preserves_multiple_projects_in_input_order() {
    let valid = task_repo();
    let broken = task_repo();
    write_raw_task(&broken, "CORE-002", "status: Pending");

    let registry = ProjectRegistry::from_roots(vec![
        valid.path().to_path_buf(),
        broken.path().to_path_buf(),
    ])
    .expect("registry builds");

    assert_eq!(
        registry.names(),
        vec![project_name(&valid), project_name(&broken)]
    );
    assert!(registry.resolve(&project_name(&valid)).is_ok());
    let broken = registry
        .resolve(&project_name(&broken))
        .expect("broken project remains registered");
    assert!(matches!(
        broken.validate(),
        Err(WebError::ProjectBroken { .. })
    ));
}

async fn app(repo: &TempDir) -> Router {
    let storage = test_storage(repo.path()).await;
    let state = AppState::single(repo.path().to_path_buf(), storage).expect("state builds");
    router(state)
}

async fn app_from_roots(roots: &[PathBuf]) -> Router {
    let storage = test_storage(&roots[0]).await;
    let state = AppState::from_roots(roots.to_vec(), storage).expect("state builds");
    router(state)
}

async fn test_storage(root: &std::path::Path) -> Storage {
    Storage::open(root.join("api-test.sqlite3"))
        .await
        .expect("test storage opens and migrates")
}

async fn get(repo: &TempDir, uri: &str) -> (StatusCode, Value) {
    let uri = project_endpoint(repo, uri);
    let request = request(&uri);
    send(repo, request).await
}

async fn post(repo: &TempDir, uri: &str, json: Option<&str>) -> (StatusCode, Value) {
    let uri = project_endpoint(repo, uri);
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

fn request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request builds")
}

async fn send(repo: &TempDir, request: Request<Body>) -> (StatusCode, Value) {
    let response = app(repo)
        .await
        .oneshot(request)
        .await
        .expect("router responds");
    response_value(response).await
}

async fn send_roots(roots: &[PathBuf], request: Request<Body>) -> (StatusCode, Value) {
    let response = app_from_roots(roots)
        .await
        .oneshot(request)
        .await
        .expect("router responds");
    response_value(response).await
}

async fn response_value(response: axum::response::Response) -> (StatusCode, Value) {
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
    create_task_repo(temp.path());
    temp
}

fn configured_repo(config: &str, domains: &[&str]) -> TempDir {
    let temp = tempfile::tempdir().expect("temp dir created");
    let tasks = temp.path().join(".tasks");
    fs::create_dir(&tasks).expect("task directory created");
    fs::write(tasks.join("config.json"), config).expect("configuration written");
    for domain in domains {
        fs::create_dir(tasks.join(domain)).expect("domain directory created");
    }
    temp
}

fn duplicate_id_repo() -> TempDir {
    configured_repo(
        r#"{
          "version": 1,
          "domains": {
            "backend": {"prefixes": ["WORK"]},
            "release": {"prefixes": ["WORK"]}
          },
          "task_types": {"Feature": {"criteria": "acceptance"}},
          "tags": {"policy": "open"}
        }"#,
        &["backend", "release"],
    )
}

fn create_task_repo(root: &std::path::Path) {
    for domain in ["core", "app", "ecosystem"] {
        fs::create_dir_all(root.join(".tasks").join(domain)).expect("domain dir created");
    }
}

fn project_name(repo: &TempDir) -> String {
    repo.path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name")
        .to_string()
}

fn project_endpoint(repo: &TempDir, suffix: &str) -> String {
    format!("/api/projects/{}{suffix}", project_name(repo))
}

fn domain_count(repo: &TempDir) -> usize {
    TaskProject::open(repo.path())
        .expect("project opens")
        .policy()
        .domains()
        .len()
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

fn write_domain_task(repo: &TempDir, domain: &str, id: &str, title: &str, extra: &str) {
    fs::write(
        repo.path()
            .join(".tasks")
            .join(domain)
            .join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n{extra}---\n\n## Acceptance Criteria\n\n- [ ] Works\n"
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
