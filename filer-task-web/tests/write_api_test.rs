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

use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};

#[tokio::test]
async fn project_task_collection_creates_a_task_and_preserves_number_text() {
    let repo = project();
    let request = create_request(&repo, "CORE", "007", Vec::new());

    let (status, body) = send(&repo, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["task"]["qualified_id"], "core:CORE-007");
    assert_eq!(body["detail"]["task"]["status"], "To Do");
    assert_eq!(body["detail"]["task"]["type"], "Feature");
    assert_eq!(
        body["detail"]["criteria"][0]["text"],
        "Define completion criteria."
    );
    assert_eq!(
        body["detail"]["criteria"][0]["content_hash"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    let path = find_task(&repo, "core", "CORE-007");
    assert!(
        path.file_name()
            .is_some_and(|name| name == "CORE-007-created-task.md")
    );
}

#[tokio::test]
async fn creation_errors_keep_library_codes_context_and_fields() {
    let duplicate = project();
    write_task(&duplicate, "CORE-007", "Existing task", "- [ ] Works\n");
    let (status, body) = send(
        &duplicate,
        create_request(&duplicate, "CORE", "007", Vec::new()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "id_exists");
    assert_eq!(body["field"], "number");
    assert_eq!(body["context"]["task"], "core:CORE-007");

    let invalid_prefix = project();
    let (status, body) = send(
        &invalid_prefix,
        create_request(&invalid_prefix, "BAD", "007", Vec::new()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "prefix_not_allowed");
    assert_eq!(body["field"], "prefix");
    assert_eq!(body["context"]["rejected_value"], "BAD");

    let strict = strict_project();
    let (status, body) = send(
        &strict,
        create_request(&strict, "CORE", "007", vec!["rejected"]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "tag_rejected");
    assert_eq!(body["field"], "tags");
    assert_eq!(body["context"]["rejected_value"], "rejected");
}

#[tokio::test]
async fn conditional_put_changes_one_marker_and_returns_the_new_hash() {
    let repo = project();
    write_task(
        &repo,
        "CORE-001",
        "Criteria task",
        "- [ ] First  \r\n  - [x] Second\r\n",
    );
    let before = fs::read_to_string(find_task(&repo, "core", "CORE-001")).expect("task read");
    let (_, shown) = send(&repo, get_task_request(&repo, "core:CORE-001")).await;
    let hash = shown["detail"]["criteria"][0]["content_hash"]
        .as_str()
        .expect("hash returned")
        .to_string();

    let (status, body) = send(
        &repo,
        criterion_request(&repo, "core:CORE-001", 0, Some(&quoted(&hash)), true),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["criteria"][0]["checked"], true);
    assert_ne!(body["detail"]["criteria"][0]["content_hash"], hash);
    assert_eq!(
        body["detail"]["criteria"][1]["content_hash"],
        shown["detail"]["criteria"][1]["content_hash"]
    );
    let after = fs::read_to_string(find_task(&repo, "core", "CORE-001")).expect("task read");
    assert_eq!(changed_byte_count(&before, &after), 1);
    assert!(after.contains("- [x] First  \r\n  - [x] Second"));
}

#[tokio::test]
async fn conditional_put_does_not_rewrite_an_already_requested_state() {
    let repo = project();
    write_task(&repo, "CORE-001", "Criteria task", "- [ ] Works\n");
    let (_, shown) = send(&repo, get_task_request(&repo, "core:CORE-001")).await;
    let hash = shown["detail"]["criteria"][0]["content_hash"]
        .as_str()
        .expect("hash returned");
    let path = find_task(&repo, "core", "CORE-001");
    let original_permissions = fs::metadata(&path).expect("metadata read").permissions();
    let mut read_only = original_permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).expect("task made read-only");

    let (status, body) = send(
        &repo,
        criterion_request(&repo, "core:CORE-001", 0, Some(&quoted(hash)), false),
    )
    .await;

    fs::set_permissions(&path, original_permissions).expect("permissions restored");
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["detail"]["criteria"][0]["checked"], false);
}

#[tokio::test]
async fn conditional_put_rejects_missing_and_malformed_preconditions_without_mutation() {
    let repo = project();
    write_task(&repo, "CORE-001", "Criteria task", "- [ ] Works\n");
    let path = find_task(&repo, "core", "CORE-001");
    let original = fs::read_to_string(&path).expect("task read");

    let (status, _) = send(
        &repo,
        criterion_request(&repo, "core:CORE-001", 0, None, true),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);

    for malformed in ["0".repeat(64), quoted("short"), quoted(&"A".repeat(64))] {
        let (status, _) = send(
            &repo,
            criterion_request(&repo, "core:CORE-001", 0, Some(&malformed), true),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "header {malformed}");
    }
    assert_eq!(fs::read_to_string(path).expect("task read"), original);
}

#[tokio::test]
async fn conditional_put_rejects_a_hash_mismatch_with_context() {
    let repo = project();
    write_task(&repo, "CORE-001", "Criteria task", "- [ ] Works\n");
    let path = find_task(&repo, "core", "CORE-001");
    let original = fs::read_to_string(&path).expect("task read");
    let expected = "0".repeat(64);

    let (status, body) = send(
        &repo,
        criterion_request(&repo, "core:CORE-001", 0, Some(&quoted(&expected)), true),
    )
    .await;

    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(body["code"], "criterion_content_mismatch");
    assert_eq!(body["context"]["expected_hash"], expected);
    assert_eq!(
        body["context"]["actual_hash"],
        "59ac2307d5d7d305aa318475ebdc4cdcaa4a97b0a7f3ad8715c86d953d6f4d24"
    );
    assert_eq!(fs::read_to_string(path).expect("task read"), original);
}

#[tokio::test]
async fn conditional_put_rejects_a_stale_hash_after_reordering() {
    let repo = project();
    write_task(
        &repo,
        "CORE-001",
        "Criteria task",
        "- [ ] First\n- [ ] Second\n",
    );
    let app = app(&repo).await;
    let shown = send_with(&app, get_task_request(&repo, "core:CORE-001"))
        .await
        .1;
    let stale = shown["detail"]["criteria"][0]["content_hash"]
        .as_str()
        .expect("hash returned");
    write_task(
        &repo,
        "CORE-001",
        "Criteria task",
        "- [ ] Second\n- [ ] First\n",
    );

    let (status, body) = send_with(
        &app,
        criterion_request(&repo, "core:CORE-001", 0, Some(&quoted(stale)), true),
    )
    .await;

    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{body}");
    assert_eq!(body["code"], "criterion_content_mismatch");
    let on_disk = fs::read_to_string(find_task(&repo, "core", "CORE-001")).expect("task read");
    assert!(on_disk.contains("- [ ] Second\n- [ ] First"));
}

#[tokio::test]
async fn criteria_writes_report_invalid_targets_without_cross_project_mutation() {
    let selected = project();
    let untouched = project();
    write_task(&selected, "CORE-001", "Criteria task", "- [ ] Works\n");
    write_task(&untouched, "CORE-001", "Criteria task", "- [ ] Works\n");
    let roots = [
        selected.path().to_path_buf(),
        untouched.path().to_path_buf(),
    ];
    let app = app_from_roots(&roots).await;
    let shown = send_with(&app, get_task_request(&selected, "core:CORE-001"))
        .await
        .1;
    let hash = shown["detail"]["criteria"][0]["content_hash"]
        .as_str()
        .expect("hash returned");

    let (status, _) = send_with(
        &app,
        criterion_request(&selected, "core:CORE-001", 9, Some(&quoted(hash)), true),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = send_with(
        &app,
        criterion_request(&selected, "core:CORE-001", 0, Some(&quoted(hash)), true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let untouched_content =
        fs::read_to_string(find_task(&untouched, "core", "CORE-001")).expect("task read");
    assert!(untouched_content.contains("- [ ] Works"));

    let unknown = Request::builder()
        .method("PUT")
        .uri("/api/projects/ghost/tasks/core:CORE-001/criteria/0")
        .header("if-match", quoted(hash))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"checked":true}"#))
        .expect("request builds");
    assert_eq!(send_with(&app, unknown).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn broken_projects_and_unscoped_write_routes_do_not_mutate() {
    let repo = project();
    write_broken_task(&repo);
    let before = fs::read_to_string(find_task(&repo, "core", "CORE-001")).expect("task read");
    let request = criterion_request(
        &repo,
        "core:CORE-001",
        0,
        Some(&quoted(&"0".repeat(64))),
        true,
    );
    let (status, _) = send(&repo, request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        fs::read_to_string(find_task(&repo, "core", "CORE-001")).expect("task read"),
        before
    );

    for request in [
        Request::builder()
            .method("POST")
            .uri("/api/tasks")
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("PUT")
            .uri("/api/tasks/core:CORE-001/criteria/0")
            .body(Body::empty())
            .expect("request builds"),
    ] {
        assert_eq!(send(&repo, request).await.0, StatusCode::NOT_FOUND);
    }
}

fn create_request(repo: &TempDir, prefix: &str, number: &str, tags: Vec<&str>) -> Request<Body> {
    let body = json!({
        "domain": "core",
        "prefix": prefix,
        "number": number,
        "title": "Created task",
        "type": "Feature",
        "priority": "High",
        "milestone": null,
        "tags": tags
    });
    Request::builder()
        .method("POST")
        .uri(project_uri(repo, "/tasks"))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

fn get_task_request(repo: &TempDir, identity: &str) -> Request<Body> {
    Request::builder()
        .uri(project_uri(repo, &format!("/tasks/{identity}")))
        .body(Body::empty())
        .expect("request builds")
}

fn criterion_request(
    repo: &TempDir,
    identity: &str,
    index: usize,
    if_match: Option<&str>,
    checked: bool,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("PUT")
        .uri(project_uri(
            repo,
            &format!("/tasks/{identity}/criteria/{index}"),
        ))
        .header(CONTENT_TYPE, "application/json");
    if let Some(value) = if_match {
        builder = builder.header("if-match", value);
    }
    builder
        .body(Body::from(json!({"checked": checked}).to_string()))
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
    app_from_roots(&[repo.path().to_path_buf()]).await
}

async fn app_from_roots(roots: &[PathBuf]) -> Router {
    let storage = Storage::open(roots[0].join("write-api-test.sqlite3"))
        .await
        .expect("storage opens");
    let state = AppState::from_roots(roots.to_vec(), storage).expect("state builds");
    router(state)
}

fn project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    for domain in ["core", "app", "ecosystem"] {
        fs::create_dir_all(repo.path().join(".tasks").join(domain)).expect("domain created");
    }
    repo
}

fn strict_project() -> TempDir {
    let repo = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(repo.path().join(".tasks")).expect("task directory created");
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

fn write_task(repo: &TempDir, id: &str, title: &str, criteria: &str) {
    let path = repo
        .path()
        .join(".tasks/core")
        .join(format!("{id}-task.md"));
    fs::write(
        path,
        format!(
            "---\nid: {id}\ntitle: {title}\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n{criteria}"
        ),
    )
    .expect("task written");
}

fn write_broken_task(repo: &TempDir) {
    let path = repo.path().join(".tasks/core/CORE-001-task.md");
    fs::write(
        path,
        "---\nid: CORE-001\ntitle: Broken task\nstatus: Pending\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("broken task written");
}

fn find_task(repo: &TempDir, domain: &str, id: &str) -> PathBuf {
    fs::read_dir(repo.path().join(".tasks").join(domain))
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

fn quoted(hash: &str) -> String {
    format!("\"{hash}\"")
}

fn changed_byte_count(left: &str, right: &str) -> usize {
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .filter(|(left, right)| left != right)
        .count()
        + left.len().abs_diff(right.len())
}
