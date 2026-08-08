use std::fs;

use axum::{
    Router,
    body::Body,
    http::{
        Request, StatusCode,
        header::{CONTENT_TYPE, USER_AGENT},
    },
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
async fn listing_sessions_requires_identity() {
    let app = app().await;

    let response = send(&app.router, request("GET", "/api/sessions", "", None, None)).await;

    assert_eq!(response.0, StatusCode::UNAUTHORIZED);
    assert_eq!(response.1["code"], "identity_required");
}

#[tokio::test]
async fn a_list_marks_the_acting_session_and_never_another_users() {
    let app = app().await;
    let alice_cookie = create_identity(&app.router, "Alice", Some(CHROME_UA)).await;
    let bob_cookie = create_identity(&app.router, "Bob", Some(FIREFOX_UA)).await;

    let alice_sessions = send(
        &app.router,
        request("GET", "/api/sessions", "", Some(&alice_cookie), None),
    )
    .await;
    assert_eq!(alice_sessions.0, StatusCode::OK, "{}", alice_sessions.1);
    let alice_rows = alice_sessions.1["sessions"]
        .as_array()
        .expect("Alice sessions array");
    assert_eq!(alice_rows.len(), 1);
    assert_eq!(alice_rows[0]["device_label"], "Chrome 131");
    assert_eq!(alice_rows[0]["current"], true);
    assert!(alice_rows[0]["created_at"].as_i64().is_some());
    assert!(alice_rows[0]["last_seen"].as_i64().is_some());

    let bob_sessions = send(
        &app.router,
        request("GET", "/api/sessions", "", Some(&bob_cookie), None),
    )
    .await;
    assert_eq!(bob_sessions.0, StatusCode::OK, "{}", bob_sessions.1);
    let bob_rows = bob_sessions.1["sessions"]
        .as_array()
        .expect("Bob sessions array");
    assert_eq!(bob_rows.len(), 1);
    assert_eq!(bob_rows[0]["device_label"], "Firefox 131");
    assert_eq!(bob_rows[0]["current"], true);
    assert_ne!(alice_rows[0]["id"], bob_rows[0]["id"]);
}

#[tokio::test]
async fn pairing_gives_the_new_browser_its_own_label() {
    let app = app().await;
    let browser_a = create_identity(&app.router, "Alice", Some(CHROME_UA)).await;
    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&browser_a), None),
    )
    .await;
    let pin = minted.1["pin"].as_str().expect("pin is a string");

    let paired = send(
        &app.router,
        request_with_ua(
            "POST",
            "/api/identity/pair",
            &json!({"username": "Alice", "pin": pin}).to_string(),
            None,
            Some(FIREFOX_UA),
        ),
    )
    .await;
    assert_eq!(paired.0, StatusCode::OK, "{}", paired.1);
    let browser_b = cookie_pair(&paired.2.expect("pairing cookie is set"));

    let listed = send(
        &app.router,
        request("GET", "/api/sessions", "", Some(&browser_b), None),
    )
    .await;
    assert_eq!(listed.0, StatusCode::OK, "{}", listed.1);
    let rows = listed.1["sessions"].as_array().expect("sessions array");
    assert_eq!(rows.len(), 2);
    let current = rows
        .iter()
        .find(|row| row["current"] == true)
        .expect("current session row");
    let other = rows
        .iter()
        .find(|row| row["current"] == false)
        .expect("other session row");
    assert_eq!(current["device_label"], "Firefox 131");
    assert_eq!(other["device_label"], "Chrome 131");
}

#[tokio::test]
async fn missing_user_agent_falls_back_to_the_placeholder() {
    let app = app().await;
    let cookie = create_identity(&app.router, "Alice", None).await;

    let listed = send(
        &app.router,
        request("GET", "/api/sessions", "", Some(&cookie), None),
    )
    .await;
    assert_eq!(listed.0, StatusCode::OK, "{}", listed.1);
    assert_eq!(listed.1["sessions"][0]["device_label"], "Unknown browser");
}

#[tokio::test]
async fn revoked_session_is_rejected_on_its_next_request() {
    let app = app().await;
    let browser_a = create_identity(&app.router, "Alice", Some(CHROME_UA)).await;
    let browser_b = pair_browser(&app.router, &browser_a, FIREFOX_UA).await;
    let listed = list(&app.router, &browser_b).await;
    let session_a = listed["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .find(|row| row["current"] == false)
        .and_then(|row| row["id"].as_i64())
        .expect("session A id");

    let revoked = send(
        &app.router,
        request(
            "DELETE",
            &format!("/api/sessions/{session_a}"),
            "",
            Some(&browser_b),
            None,
        ),
    )
    .await;
    assert_eq!(revoked.0, StatusCode::NO_CONTENT, "{}", revoked.1);

    let rejected = send(
        &app.router,
        request("GET", "/api/identity", "", Some(&browser_a), None),
    )
    .await;
    assert_eq!(rejected.0, StatusCode::UNAUTHORIZED);
    assert_eq!(rejected.1["code"], "identity_required");
}

#[tokio::test]
async fn revoking_a_session_invalidates_the_pins_it_minted() {
    let app = app().await;
    let browser_a = create_identity(&app.router, "Alice", Some(CHROME_UA)).await;
    let browser_b = pair_browser(&app.router, &browser_a, FIREFOX_UA).await;
    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&browser_a), None),
    )
    .await;
    let pin = minted.1["pin"]
        .as_str()
        .expect("pin is a string")
        .to_string();
    let listed = list(&app.router, &browser_b).await;
    let session_a = listed["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .find(|row| row["current"] == false)
        .and_then(|row| row["id"].as_i64())
        .expect("session A id");

    let revoked = send(
        &app.router,
        request(
            "DELETE",
            &format!("/api/sessions/{session_a}"),
            "",
            Some(&browser_b),
            None,
        ),
    )
    .await;
    assert_eq!(revoked.0, StatusCode::NO_CONTENT, "{}", revoked.1);

    let pair_attempt = send(
        &app.router,
        request(
            "POST",
            "/api/identity/pair",
            &json!({"username": "Alice", "pin": pin}).to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(pair_attempt.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(pair_attempt.1["code"], "pairing_pin_wrong");
}

#[tokio::test]
async fn self_revoke_is_rejected() {
    let app = app().await;
    let cookie = create_identity(&app.router, "Alice", Some(CHROME_UA)).await;
    let listed = list(&app.router, &cookie).await;
    let session_id = listed["sessions"][0]["id"]
        .as_i64()
        .expect("current session id");

    let rejected = send(
        &app.router,
        request(
            "DELETE",
            &format!("/api/sessions/{session_id}"),
            "",
            Some(&cookie),
            None,
        ),
    )
    .await;
    assert_eq!(rejected.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(rejected.1["code"], "session_revoke_current");
    assert_eq!(
        send(
            &app.router,
            request("GET", "/api/identity", "", Some(&cookie), None),
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn revoking_another_users_session_returns_not_found() {
    let app = app().await;
    let alice_cookie = create_identity(&app.router, "Alice", Some(CHROME_UA)).await;
    let bob_cookie = create_identity(&app.router, "Bob", Some(FIREFOX_UA)).await;
    let bob_sessions = list(&app.router, &bob_cookie).await;
    let bob_session_id = bob_sessions["sessions"][0]["id"]
        .as_i64()
        .expect("Bob session id");

    let rejected = send(
        &app.router,
        request(
            "DELETE",
            &format!("/api/sessions/{bob_session_id}"),
            "",
            Some(&alice_cookie),
            None,
        ),
    )
    .await;
    assert_eq!(rejected.0, StatusCode::NOT_FOUND);
    assert_eq!(rejected.1["code"], "session_not_found");
    assert_eq!(
        list(&app.router, &bob_cookie).await["sessions"]
            .as_array()
            .expect("Bob sessions")
            .len(),
        1
    );
}

const CHROME_UA: &str = "Mozilla/5.0 Chrome/131.0.0.0 Safari/537.36";
const FIREFOX_UA: &str = "Mozilla/5.0 Firefox/131.0";

async fn pair_browser(router: &Router, source_cookie: &str, user_agent: &str) -> String {
    let minted = send(
        router,
        request("POST", "/api/identity/pin", "", Some(source_cookie), None),
    )
    .await;
    assert_eq!(minted.0, StatusCode::OK, "{}", minted.1);
    let pin = minted.1["pin"].as_str().expect("pin is a string");
    let paired = send(
        router,
        request_with_ua(
            "POST",
            "/api/identity/pair",
            &json!({"username": "Alice", "pin": pin}).to_string(),
            None,
            Some(user_agent),
        ),
    )
    .await;
    assert_eq!(paired.0, StatusCode::OK, "{}", paired.1);
    cookie_pair(&paired.2.expect("pairing cookie is set"))
}

async fn create_identity(router: &Router, username: &str, user_agent: Option<&str>) -> String {
    let created = send(
        router,
        request_with_ua(
            "PUT",
            "/api/identity",
            &json!({"username": username}).to_string(),
            None,
            user_agent,
        ),
    )
    .await;
    assert_eq!(created.0, StatusCode::OK, "{}", created.1);
    cookie_pair(&created.2.expect("identity cookie is set"))
}

async fn list(router: &Router, cookie: &str) -> Value {
    let response = send(
        router,
        request("GET", "/api/sessions", "", Some(cookie), None),
    )
    .await;
    assert_eq!(response.0, StatusCode::OK, "{}", response.1);
    response.1
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

fn request(
    method: &str,
    uri: &str,
    body: &str,
    cookie: Option<&str>,
    user_agent: Option<&str>,
) -> Request<Body> {
    request_with_ua(method, uri, body, cookie, user_agent)
}

fn request_with_ua(
    method: &str,
    uri: &str,
    body: &str,
    cookie: Option<&str>,
    user_agent: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(user_agent) = user_agent {
        builder = builder.header(USER_AGENT, user_agent);
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

struct TestApp {
    router: Router,
    _storage: Storage,
    _repo: TempDir,
}

async fn app() -> TestApp {
    let repo = tempfile::tempdir().expect("project creates");
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("task directory creates");
    let storage = Storage::open(repo.path().join("sessions-api-test.sqlite3"))
        .await
        .expect("storage opens");
    let state = AppState::single(repo.path().to_path_buf(), storage.clone()).expect("state builds");
    TestApp {
        router: router(state),
        _storage: storage,
        _repo: repo,
    }
}
