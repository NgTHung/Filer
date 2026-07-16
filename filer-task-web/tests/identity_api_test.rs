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

use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};

#[tokio::test]
async fn identity_api_handles_onboarding_rename_validation_and_stale_recovery() {
    let repo = project();
    let app = app(&repo).await;
    let anonymous = send(&app, request("GET", "/api/identity", "", None)).await;
    assert_eq!(anonymous.0, StatusCode::UNAUTHORIZED);
    assert_eq!(anonymous.1["code"], "identity_required");

    for username in [
        "   ".to_string(),
        "bad\u{0007}name".to_string(),
        "a".repeat(65),
    ] {
        let invalid = put_username(&app, &username, None).await;
        assert_eq!(invalid.0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(invalid.1["code"], "invalid_username");
        assert_eq!(invalid.1["field"], "username");
    }

    let created = put_username(&app, "  Alice  ", None).await;
    assert_eq!(created.0, StatusCode::OK, "{}", created.1);
    assert_eq!(created.1, json!({"username": "Alice"}));
    let set_cookie = created.2.expect("identity cookie is set");
    for attribute in ["Max-Age=31536000", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(set_cookie.contains(attribute), "missing {attribute}");
    }
    assert!(!set_cookie.contains("Secure"));
    let cookie = cookie_pair(&set_cookie);
    assert_eq!(
        send(&app, request("GET", "/api/identity", "", Some(&cookie)))
            .await
            .1,
        json!({"username": "Alice"})
    );

    let renamed = put_username(&app, "Alice Cooper", Some(&cookie)).await;
    assert_eq!(renamed.1["username"], "Alice Cooper");
    assert_eq!(
        cookie_pair(renamed.2.as_deref().expect("cookie refreshed")),
        cookie
    );
    let duplicate = put_username(&app, "ALICE COOPER", None).await;
    assert_eq!(duplicate.0, StatusCode::CONFLICT);
    assert_eq!(duplicate.1["code"], "username_taken");
    assert_eq!(duplicate.1["field"], "username");

    let stale = format!("filer_task_identity={}", "0".repeat(64));
    assert_eq!(
        send(&app, request("GET", "/api/identity", "", Some(&stale)))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let recovered = put_username(&app, "Recovered", Some(&stale)).await;
    assert_eq!(recovered.0, StatusCode::OK);
    let fresh = cookie_pair(recovered.2.as_deref().expect("fresh cookie set"));
    assert_ne!(fresh, stale);
    assert_eq!(
        send(&app, request("GET", "/api/identity", "", Some(&fresh)))
            .await
            .1["username"],
        "Recovered"
    );
}

#[tokio::test]
async fn every_existing_write_requires_identity_before_mutation() {
    let repo = project();
    write_task(&repo);
    let app = app(&repo).await;
    let name = project_name(&repo);
    let task_path = repo.path().join(".tasks/core/CORE-001-task.md");
    let original = fs::read(&task_path).expect("task reads");
    let task = format!("/api/projects/{name}/tasks/core:CORE-001");
    let routes = [
        ("POST", "/api/projects".to_string()),
        ("DELETE", format!("/api/projects/{name}")),
        ("PATCH", format!("/api/projects/{name}/policy")),
        ("POST", format!("/api/projects/{name}/tasks")),
        ("PATCH", task.clone()),
        ("PUT", format!("{task}/criteria/0")),
        ("POST", format!("{task}/start")),
        ("POST", format!("{task}/done")),
        ("POST", format!("{task}/block")),
        ("POST", format!("{task}/defer")),
        ("POST", format!("{task}/obsolete")),
    ];
    for cookie in [
        None,
        Some(format!("filer_task_identity={}", "f".repeat(64))),
    ] {
        for (method, uri) in &routes {
            let response = send(&app, request(method, uri, "{}", cookie.as_deref())).await;
            assert_eq!(response.0, StatusCode::UNAUTHORIZED, "{method} {uri}");
            assert_eq!(response.1["code"], "identity_required");
        }
    }
    assert_eq!(
        fs::read(task_path).expect("task reads after requests"),
        original
    );
}

async fn put_username(app: &Router, username: &str, cookie: Option<&str>) -> TestResponse {
    send(
        app,
        request(
            "PUT",
            "/api/identity",
            &json!({"username": username}).to_string(),
            cookie,
        ),
    )
    .await
}

type TestResponse = (StatusCode, Value, Option<String>);

async fn send(app: &Router, request: Request<Body>) -> TestResponse {
    let response = app.clone().oneshot(request).await.expect("router responds");
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    )
}

fn request(method: &str, uri: &str, body: &str, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

async fn app(repo: &TempDir) -> Router {
    let storage = Storage::open(repo.path().join("identity-api-test.sqlite3"))
        .await
        .expect("storage opens");
    router(AppState::single(repo.path().to_path_buf(), storage).expect("state builds"))
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

fn project() -> TempDir {
    let repo = tempfile::tempdir().expect("project creates");
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("task directory creates");
    repo
}

fn project_name(repo: &TempDir) -> String {
    repo.path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable name")
        .to_string()
}

fn write_task(repo: &TempDir) {
    fs::write(
        repo.path().join(".tasks/core/CORE-001-task.md"),
        "---\nid: CORE-001\ntitle: Identity guard\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Protected\n",
    )
    .expect("task writes");
}
