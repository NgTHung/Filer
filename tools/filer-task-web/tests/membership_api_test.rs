//! Exercises project membership over HTTP: role guards on every write route,
//! member and access-request management, and the role fields the browser reads.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    body::Body,
    http::{
        Request, StatusCode,
        header::{CONTENT_TYPE, COOKIE},
    },
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::{
    app::{AppState, router},
    identity::IDENTITY_COOKIE,
    storage::{Role, Storage},
};

const SEED_TASK: &str = "---\nid: CORE-001\ntitle: Seed task\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n";

#[tokio::test]
async fn every_guarded_route_refuses_strangers_before_touching_files() {
    let app = app().await;
    let (_, owner_id) = app.user("Olivia", Some(Role::Owner)).await;
    let (stranger, _) = app.user("Sam", None).await;
    let before = app.task_files();

    for (method, uri, guard) in write_routes(&app, owner_id) {
        if guard == Guard::Open {
            continue;
        }
        let (status, body) = send(&app.router, request(method, &uri, None, None)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {uri} without a session: {body}"
        );
        if guard == Guard::Session {
            continue;
        }
        let (status, body) = send(&app.router, request(method, &uri, None, Some(&stranger))).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {uri} as a non-member: {body}"
        );
        let expected = if guard == Guard::Admin {
            "admin_required"
        } else {
            "project_role_required"
        };
        assert_eq!(body["code"], expected, "{method} {uri}");
    }

    assert_eq!(
        app.task_files(),
        before,
        "no refused request changed a task file"
    );
}

#[tokio::test]
async fn editors_write_tasks_but_not_owner_routes() {
    let app = app().await;
    app.user("Olivia", Some(Role::Owner)).await;
    let (editor, _) = app.user("Eve", Some(Role::Editor)).await;

    let (status, body) = send(
        &app.router,
        request(
            "POST",
            &app.uri("/tasks/core:CORE-001/start"),
            None,
            Some(&editor),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["detail"]["task"]["status"], "In Progress");

    for (method, suffix) in [("PATCH", "/policy"), ("DELETE", "")] {
        let (status, body) = send(
            &app.router,
            request(method, &app.uri(suffix), None, Some(&editor)),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {suffix}: {body}");
        assert_eq!(body["code"], "project_role_required");
    }
}

#[tokio::test]
async fn only_admins_register_projects_and_registrants_become_owners() {
    let app = app().await;
    let (admin, admin_id) = app.user("Olivia", Some(Role::Owner)).await;
    let (member, _) = app.user("Eve", Some(Role::Editor)).await;
    let other = tempfile::tempdir().expect("second project created");
    fs::create_dir(other.path().join(".tasks")).expect("task directory created");
    let body = json!({"path": other.path()});

    let (status, refused) = send(
        &app.router,
        request("POST", "/api/projects", Some(body.clone()), Some(&member)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    assert_eq!(refused["code"], "admin_required");

    let (status, summary) = send(
        &app.router,
        request("POST", "/api/projects", Some(body), Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    let name = summary["name"].as_str().expect("summary names the project");
    assert_eq!(
        app.storage
            .project_role(name, admin_id)
            .await
            .expect("role reads"),
        Some(Role::Owner)
    );
}

#[tokio::test]
async fn the_guard_matches_a_project_name_that_needs_percent_encoding() {
    let database = tempfile::tempdir().expect("database directory created");
    let parent = tempfile::tempdir().expect("parent created");
    let root = parent.path().join("My Project");
    fs::create_dir_all(root.join(".tasks/core")).expect("domain created");
    fs::write(root.join(".tasks/core/CORE-001-task.md"), SEED_TASK).expect("task written");
    let storage = Storage::open(database.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let state = AppState::single(root, storage.clone()).expect("state builds");
    let registration = state
        .project_registrations()
        .into_iter()
        .next()
        .expect("one project");
    assert_eq!(registration.name, "My Project");
    storage
        .insert_project_registration(&registration)
        .await
        .expect("registration persists");
    let session = storage
        .create_identity("Eve", "Test browser")
        .await
        .expect("identity creates");
    storage
        .grant_member(
            "My Project",
            session.identity.user_id,
            Role::Editor,
            None,
            false,
        )
        .await
        .expect("editor grants");
    let cookie = format!("{IDENTITY_COOKIE}={}", session.session_token);

    let (status, body) = send(
        &router(state),
        request(
            "POST",
            "/api/projects/My%20Project/tasks/core:CORE-001/start",
            None,
            Some(&cookie),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
}

struct TestApp {
    router: Router,
    storage: Storage,
    project: String,
    repo: TempDir,
    _database: TempDir,
}

/// One registered project with one task. The project registration is
/// persisted so memberships can reference it.
async fn app() -> TestApp {
    let database = tempfile::tempdir().expect("database directory created");
    let repo = tempfile::tempdir().expect("project created");
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("domain created");
    fs::write(repo.path().join(".tasks/core/CORE-001-task.md"), SEED_TASK).expect("task written");
    let storage = Storage::open(database.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    let state = AppState::single(repo.path().to_path_buf(), storage.clone()).expect("state builds");
    let registration = state
        .project_registrations()
        .into_iter()
        .next()
        .expect("one project");
    storage
        .insert_project_registration(&registration)
        .await
        .expect("registration persists");
    TestApp {
        router: router(state),
        storage,
        project: registration.name,
        repo,
        _database: database,
    }
}

impl TestApp {
    /// Creates a user and returns their cookie and id. The first user created
    /// on a database becomes admin.
    async fn user(&self, username: &str, role: Option<Role>) -> (String, i64) {
        let session = self
            .storage
            .create_identity(username, "Test browser")
            .await
            .expect("identity creates");
        if let Some(role) = role {
            self.storage
                .grant_member(&self.project, session.identity.user_id, role, None, false)
                .await
                .expect("membership grants");
        }
        (
            format!("{IDENTITY_COOKIE}={}", session.session_token),
            session.identity.user_id,
        )
    }

    fn uri(&self, suffix: &str) -> String {
        format!("/api/projects/{}{suffix}", self.project)
    }

    fn task_files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        snapshot(&self.repo.path().join(".tasks"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Guard {
    Admin,
    Editor,
    Owner,
    Session,
    Open,
}

/// Every non-GET route the router serves, with the guard it enforces. The
/// comment above `app::router` points here: a non-GET route joins this table.
fn write_routes(app: &TestApp, _other_user: i64) -> Vec<(&'static str, String, Guard)> {
    vec![
        ("PUT", "/api/identity".to_string(), Guard::Open),
        ("POST", "/api/identity/pair".to_string(), Guard::Open),
        ("POST", "/api/identity/pin".to_string(), Guard::Session),
        ("DELETE", "/api/sessions/1".to_string(), Guard::Session),
        ("POST", "/api/projects".to_string(), Guard::Admin),
        ("DELETE", app.uri(""), Guard::Owner),
        ("PATCH", app.uri("/policy"), Guard::Owner),
        ("POST", app.uri("/tasks"), Guard::Editor),
        ("PATCH", app.uri("/tasks/core:CORE-001"), Guard::Editor),
        ("PUT", app.uri("/tasks/core:CORE-001/criteria/0"), Guard::Editor),
        ("POST", app.uri("/tasks/core:CORE-001/start"), Guard::Editor),
        ("POST", app.uri("/tasks/core:CORE-001/done"), Guard::Editor),
        ("POST", app.uri("/tasks/core:CORE-001/block"), Guard::Editor),
        ("POST", app.uri("/tasks/core:CORE-001/defer"), Guard::Editor),
        ("POST", app.uri("/tasks/core:CORE-001/obsolete"), Guard::Editor),
    ]
}

fn request(method: &str, uri: &str, body: Option<Value>, cookie: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        builder = builder.header(COOKIE, cookie);
    }
    let body = match body {
        Some(body) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    builder.body(body).expect("request builds")
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

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("directory reads") {
            let path = entry.expect("entry reads").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let contents = fs::read(&path).expect("file reads");
                files.insert(path, contents);
            }
        }
    }
    files
}
