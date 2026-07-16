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

mod common;

#[tokio::test]
async fn empty_database_starts_with_an_empty_registry() {
    let database = tempfile::tempdir().expect("database directory created");
    let storage = Storage::open(database.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let app = router(AppState::load(storage).await.expect("state loads"));

    let (status, projects) = send(&app, request("GET", "/api/projects", None)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(projects, json!([]));
}

#[tokio::test]
async fn registered_project_survives_a_server_restart() {
    let database = tempfile::tempdir().expect("database directory created");
    let database_path = database.path().join("state.sqlite3");
    let project = project();
    let project_name = project_name(&project);

    let (app, storage) = open_app(&database_path).await;
    let (status, _) = send(
        &app,
        request(
            "POST",
            "/api/projects",
            Some(json!({"path": project.path()})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    drop(app);
    storage.close().await;

    let (restarted, _) = open_app(&database_path).await;
    let (status, projects) = send(&restarted, request("GET", "/api/projects", None)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(projects.as_array().map(Vec::len), Some(1));
    assert_eq!(projects[0]["name"], project_name);
    assert_eq!(projects[0]["broken"], false);
}

#[tokio::test]
async fn vanished_persisted_root_is_visible_as_broken_after_restart() {
    let database = tempfile::tempdir().expect("database directory created");
    let database_path = database.path().join("state.sqlite3");
    let project = project();
    let root = project
        .path()
        .canonicalize()
        .expect("project root canonicalizes");
    let project_name = project_name(&project);

    let (app, storage) = open_app(&database_path).await;
    assert_eq!(
        send(
            &app,
            request(
                "POST",
                "/api/projects",
                Some(json!({"path": project.path()})),
            ),
        )
        .await
        .0,
        StatusCode::OK
    );
    drop(app);
    storage.close().await;
    fs::remove_dir_all(&root).expect("project root removed");

    let (restarted, _) = open_app(&database_path).await;
    let (status, projects) = send(&restarted, request("GET", "/api/projects", None)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(projects.as_array().map(Vec::len), Some(1));
    assert_eq!(projects[0]["name"], project_name);
    assert_eq!(projects[0]["task_count"], 0);
    assert_eq!(projects[0]["domain_count"], 0);
    assert_eq!(projects[0]["broken"], true);
    assert_eq!(projects[0]["issues"][0]["code"], "io");
    assert_eq!(projects[0]["issues"][0]["path"], json!(root));
}

#[tokio::test]
async fn deleted_project_stays_absent_after_restart() {
    let database = tempfile::tempdir().expect("database directory created");
    let database_path = database.path().join("state.sqlite3");
    let project = project();
    let project_name = project_name(&project);
    let (app, storage) = open_app(&database_path).await;
    assert_eq!(
        send(
            &app,
            request(
                "POST",
                "/api/projects",
                Some(json!({"path": project.path()})),
            ),
        )
        .await
        .0,
        StatusCode::OK
    );

    let (status, body) = send(
        &app,
        request("DELETE", &format!("/api/projects/{project_name}"), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);
    let (_, projects) = send(&app, request("GET", "/api/projects", None)).await;
    assert_eq!(projects, json!([]));
    drop(app);
    storage.close().await;

    let (restarted, _) = open_app(&database_path).await;
    let (_, projects) = send(&restarted, request("GET", "/api/projects", None)).await;
    assert_eq!(projects, json!([]));
}

#[tokio::test]
async fn deleting_an_unknown_project_returns_not_found() {
    let database = tempfile::tempdir().expect("database directory created");
    let (app, _) = open_app(&database.path().join("state.sqlite3")).await;

    let (status, body) = send(&app, request("DELETE", "/api/projects/ghost", None)).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| error.contains("ghost"))
    );
}

#[tokio::test]
async fn failed_persistence_does_not_publish_the_project_in_memory() {
    let database = tempfile::tempdir().expect("database directory created");
    let project = project();
    let (app, storage) = open_app(&database.path().join("state.sqlite3")).await;
    storage.close().await;

    let (status, _) = send(
        &app,
        request(
            "POST",
            "/api/projects",
            Some(json!({"path": project.path()})),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (_, projects) = send(&app, request("GET", "/api/projects", None)).await;
    assert_eq!(projects, json!([]));
}

#[tokio::test]
async fn failed_deregistration_keeps_the_project_in_memory() {
    let database = tempfile::tempdir().expect("database directory created");
    let project = project();
    let project_name = project_name(&project);
    let (app, storage) = open_app(&database.path().join("state.sqlite3")).await;
    assert_eq!(
        send(
            &app,
            request(
                "POST",
                "/api/projects",
                Some(json!({"path": project.path()})),
            ),
        )
        .await
        .0,
        StatusCode::OK
    );
    storage.close().await;

    let (status, _) = send(
        &app,
        request("DELETE", &format!("/api/projects/{project_name}"), None),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (_, projects) = send(&app, request("GET", "/api/projects", None)).await;
    assert_eq!(projects.as_array().map(Vec::len), Some(1));
    assert_eq!(projects[0]["name"], project_name);
}

async fn open_app(database_path: &std::path::Path) -> (Router, Storage) {
    let storage = Storage::open(database_path).await.expect("storage opens");
    let state = AppState::load(storage.clone()).await.expect("state loads");
    (common::authenticated_router(state).await, storage)
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
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("response is JSON")
    };
    (status, body)
}

fn request(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(body) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).expect("request builds")
}

fn project() -> TempDir {
    let project = tempfile::tempdir().expect("temporary project created");
    fs::create_dir(project.path().join(".tasks")).expect("task directory created");
    project
}

fn project_name(project: &TempDir) -> String {
    project
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name")
        .to_string()
}
