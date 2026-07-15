use std::fs;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};

#[tokio::test]
async fn routes_return_empty_arrays() {
    let repo = compatibility_project();

    for suffix in ["/ready", "/milestones"] {
        let (status, body) = get(&repo, suffix).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!([]));
    }
}

#[tokio::test]
async fn ready_applies_readiness_ordering_and_filters() {
    let repo = compatibility_project();
    write_milestone(&repo, "MILESTONE-001", "1.0.0");
    for task in [
        TaskFixture::feature("core", "CORE-001", "Completed dependency", "Done", "High")
            .with_extra("milestone: \"1.0.0\"\n")
            .checked(),
        TaskFixture::feature("core", "CORE-002", "Waiting task", "To Do", "High")
            .with_extra("depends_on: [CORE-003]\nmilestone: \"1.0.0\"\n"),
        TaskFixture::feature(
            "core",
            "CORE-003",
            "Unfinished dependency",
            "In Progress",
            "High",
        )
        .with_extra("milestone: \"1.0.0\"\n"),
        TaskFixture::feature("core", "CORE-004", "Low priority ready", "To Do", "Low")
            .with_extra("depends_on: [CORE-001]\nmilestone: \"1.0.0\"\n"),
        TaskFixture::feature("core", "CORE-010", "High priority ready", "To Do", "High")
            .with_extra("depends_on: [CORE-001]\nmilestone: \"1.0.0\"\n"),
        TaskFixture::feature("app", "UI-001", "App ready task", "To Do", "High")
            .with_extra("milestone: \"1.0.0\"\n"),
        TaskFixture::feature("core", "CORE-020", "Blocked task", "Blocked", "High")
            .with_extra("milestone: \"1.0.0\"\n"),
    ] {
        task.write(&repo);
    }

    let (status, body) = get(&repo, "/ready").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        qualified_ids(&body),
        ["app:UI-001", "core:CORE-010", "core:CORE-004"]
    );

    let (_, domain) = get(&repo, "/ready?domain=core").await;
    assert_eq!(qualified_ids(&domain), ["core:CORE-010", "core:CORE-004"]);

    let (_, milestone) = get(&repo, "/ready?milestone=1.0.0").await;
    assert_eq!(
        qualified_ids(&milestone),
        ["app:UI-001", "core:CORE-010", "core:CORE-004"]
    );
    let (_, missing) = get(&repo, "/ready?milestone=9.9.9").await;
    assert_eq!(missing, serde_json::json!([]));
}

#[tokio::test]
async fn milestones_aggregate_configured_roles_and_status_groups() {
    let repo = configured_project();
    for task in [
        TaskFixture::new(
            "release",
            "REL-002",
            "Second release",
            "To Do",
            "High",
            "ReleaseGate",
        )
        .with_extra("milestone: \"2.0.0\"\n"),
        TaskFixture::new(
            "release",
            "REL-001",
            "First release",
            "In Progress",
            "High",
            "ReleaseGate",
        )
        .with_extra("milestone: \"1.0.0\"\n"),
        TaskFixture::new(
            "backend",
            "WORK-002",
            "Finished task",
            "Done",
            "High",
            "Change",
        )
        .with_extra("milestone: \"1.0.0\"\n")
        .checked(),
        TaskFixture::new(
            "backend",
            "WORK-001",
            "Blocked task",
            "Blocked",
            "High",
            "Change",
        )
        .with_extra("milestone: \"1.0.0\"\n"),
    ] {
        task.write(&repo);
    }

    let (status, body) = get(&repo, "/milestones").await;

    assert_eq!(status, StatusCode::OK);
    let milestones = body.as_array().expect("milestones array");
    assert_eq!(milestones.len(), 2);
    assert_eq!(
        milestones[0]["milestone"]["qualified_id"],
        "release:REL-001"
    );
    assert_eq!(milestones[0]["criteria_heading"], "Acceptance Criteria");
    assert_eq!(
        milestones[0]["criteria"],
        serde_json::json!([{"checked": false, "text": "Works"}])
    );
    assert_eq!(milestones[0]["done"], 1);
    assert_eq!(milestones[0]["total"], 3);
    assert_eq!(
        milestones[0]["tasks_by_status"]["In Progress"][0]["qualified_id"],
        "release:REL-001"
    );
    assert_eq!(
        milestones[0]["tasks_by_status"]["Blocked"][0]["qualified_id"],
        "backend:WORK-001"
    );
    assert_eq!(
        milestones[0]["tasks_by_status"]["Done"][0]["qualified_id"],
        "backend:WORK-002"
    );
    assert!(milestones[0]["tasks_by_status"].get("To Do").is_none());
    assert_eq!(
        milestones[1]["milestone"]["qualified_id"],
        "release:REL-002"
    );
    assert_eq!(milestones[1]["total"], 1);
}

#[tokio::test]
async fn routes_are_project_scoped_and_validate_fresh_state() {
    let repo = compatibility_project();

    for route in ["/api/ready", "/api/milestones"] {
        let (status, _) = send(&repo, route).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    fs::write(
        repo.path().join(".tasks/core/CORE-001-task.md"),
        "---\nid: CORE-001\ntitle: Broken task\nstatus: Pending\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n",
    )
    .expect("broken task written");
    for suffix in ["/ready", "/milestones"] {
        let (status, body) = get(&repo, suffix).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["project"], project_name(&repo));
    }
}

struct TaskFixture<'a> {
    domain: &'a str,
    id: &'a str,
    title: &'a str,
    status: &'a str,
    priority: &'a str,
    task_type: &'a str,
    extra: &'a str,
    checked: bool,
}

impl<'a> TaskFixture<'a> {
    fn new(
        domain: &'a str,
        id: &'a str,
        title: &'a str,
        status: &'a str,
        priority: &'a str,
        task_type: &'a str,
    ) -> Self {
        Self {
            domain,
            id,
            title,
            status,
            priority,
            task_type,
            extra: "",
            checked: false,
        }
    }

    fn feature(
        domain: &'a str,
        id: &'a str,
        title: &'a str,
        status: &'a str,
        priority: &'a str,
    ) -> Self {
        Self::new(domain, id, title, status, priority, "Feature")
    }

    fn with_extra(mut self, extra: &'a str) -> Self {
        self.extra = extra;
        self
    }

    fn checked(mut self) -> Self {
        self.checked = true;
        self
    }

    fn write(&self, repo: &TempDir) {
        let directory = repo.path().join(".tasks").join(self.domain);
        fs::create_dir_all(&directory).expect("domain created");
        let marker = if self.checked { "x" } else { " " };
        let blocked_reason = if self.status == "Blocked" {
            "\n## Blocked Reason\n\nWaiting for input.\n"
        } else {
            ""
        };
        fs::write(
            directory.join(format!("{}-task.md", self.id)),
            format!(
                "---\nid: {}\ntitle: {}\nstatus: {}\npriority: {}\ntype: {}\n{}---\n\n## Acceptance Criteria\n\n- [{marker}] Works\n{blocked_reason}",
                self.id, self.title, self.status, self.priority, self.task_type, self.extra
            ),
        )
        .expect("task written");
    }
}

fn compatibility_project() -> TempDir {
    let repo = tempfile::tempdir().expect("temp dir created");
    for domain in ["core", "app", "ecosystem"] {
        fs::create_dir_all(repo.path().join(".tasks").join(domain)).expect("domain created");
    }
    repo
}

fn configured_project() -> TempDir {
    let repo = tempfile::tempdir().expect("temp dir created");
    fs::create_dir(repo.path().join(".tasks")).expect("task dir created");
    fs::write(
        repo.path().join(".tasks/config.json"),
        r#"{
  "version": 1,
  "domains": {
    "backend": {"prefixes": ["WORK"]},
    "release": {"prefixes": ["REL"]}
  },
  "task_types": {
    "Change": {"criteria": "acceptance"},
    "ReleaseGate": {"criteria": "acceptance", "role": "milestone"}
  },
  "tags": {"policy": "open"}
}"#,
    )
    .expect("configuration written");
    repo
}

fn write_milestone(repo: &TempDir, id: &str, milestone: &str) {
    let directory = repo.path().join(".tasks/milestones");
    fs::create_dir_all(&directory).expect("milestone directory created");
    fs::write(
        directory.join(format!("{id}-task.md")),
        format!(
            "---\nid: {id}\ntitle: Project milestone\nstatus: To Do\npriority: High\ntype: Milestone\nmilestone: \"{milestone}\"\n---\n\n## Exit Criteria\n\n- [ ] Ships\n"
        ),
    )
    .expect("milestone written");
}

async fn get(repo: &TempDir, suffix: &str) -> (StatusCode, Value) {
    let uri = format!("/api/projects/{}{suffix}", project_name(repo));
    send(repo, &uri).await
}

async fn send(repo: &TempDir, uri: &str) -> (StatusCode, Value) {
    let response = app(repo)
        .await
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn app(repo: &TempDir) -> Router {
    let storage = Storage::open(repo.path().join("api-test.sqlite3"))
        .await
        .expect("test storage opens");
    let state = AppState::single(repo.path().to_path_buf(), storage).expect("state builds");
    router(state)
}

fn project_name(repo: &TempDir) -> &str {
    repo.path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("portable project name")
}

fn qualified_ids(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("task array")
        .iter()
        .map(|task| task["qualified_id"].as_str().expect("qualified id"))
        .collect()
}
