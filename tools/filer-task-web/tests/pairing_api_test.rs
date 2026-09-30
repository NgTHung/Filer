use std::{fs, path::Path, time::UNIX_EPOCH};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::TempDir;
use tower::ServiceExt;

use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};

#[tokio::test]
async fn minting_a_pairing_pin_requires_an_authenticated_browser() {
    let app = app().await;
    let response = send(
        &app.router,
        request("POST", "/api/identity/pin", "{}", None),
    )
    .await;
    assert_eq!(response.0, StatusCode::UNAUTHORIZED);
    assert_eq!(response.1["code"], "identity_required");
}

#[tokio::test]
async fn an_authenticated_browser_mints_a_six_digit_pairing_pin() {
    let app = app().await;
    let cookie = create_identity(&app.router, "Alice").await;

    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&cookie)),
    )
    .await;
    assert_eq!(minted.0, StatusCode::OK, "{}", minted.1);
    let pin = minted.1["pin"].as_str().expect("pin is a string");
    assert_eq!(pin.len(), 6);
    assert!(pin.chars().all(|digit| digit.is_ascii_digit()));
    assert!(
        minted.1["expires_at"].as_i64().expect("expires_at reads") > unix_now(),
        "pin is not already expired"
    );
}

#[tokio::test]
async fn pairing_adopts_the_identity_into_a_fresh_browser_with_its_own_session() {
    let app = app().await;
    let browser_a = create_identity(&app.router, "Alice").await;
    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&browser_a)),
    )
    .await;
    let pin = minted.1["pin"]
        .as_str()
        .expect("pin is a string")
        .to_string();

    let paired = send(
        &app.router,
        request(
            "POST",
            "/api/identity/pair",
            &json!({"username": "Alice", "pin": pin}).to_string(),
            None,
        ),
    )
    .await;
    assert_eq!(paired.0, StatusCode::OK, "{}", paired.1);
    assert_eq!(paired.1, json!({"username": "Alice"}));
    let set_cookie = paired.2.as_deref().expect("pairing cookie is set");
    for attribute in ["Max-Age=31536000", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(set_cookie.contains(attribute), "missing {attribute}");
    }
    let browser_b = cookie_pair(set_cookie);
    assert_ne!(browser_b, browser_a);

    assert_eq!(
        send(
            &app.router,
            request("GET", "/api/identity", "", Some(&browser_b))
        )
        .await
        .1,
        json!({"username": "Alice"})
    );
    assert_eq!(
        send(
            &app.router,
            request("GET", "/api/identity", "", Some(&browser_a))
        )
        .await
        .1,
        json!({"username": "Alice"})
    );

    let session_a = app
        .storage
        .resolve_identity(&cookie_pair_value(&browser_a))
        .await
        .expect("session A resolves")
        .expect("session A exists");
    let session_b = app
        .storage
        .resolve_identity(&cookie_pair_value(&browser_b))
        .await
        .expect("session B resolves")
        .expect("session B exists");
    assert_eq!(session_a.identity.user_id, session_b.identity.user_id);
    assert_ne!(session_a.session_id, session_b.session_id);
}

#[tokio::test]
async fn pairing_rejects_unknown_usernames_and_wrong_pins_with_distinct_codes() {
    let app = app().await;
    let cookie = create_identity(&app.router, "Alice").await;
    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&cookie)),
    )
    .await;
    let pin = minted.1["pin"]
        .as_str()
        .expect("pin is a string")
        .to_string();

    let unknown = pair(&app.router, "Nobody", &pin).await;
    assert_eq!(unknown.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(unknown.1["code"], "pairing_username_unknown");
    assert_eq!(unknown.1["field"], "username");

    let wrong = "999999";
    let wrong = if wrong == pin { "000000" } else { wrong };
    let rejected = pair(&app.router, "Alice", wrong).await;
    assert_eq!(rejected.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(rejected.1["code"], "pairing_pin_wrong");
    assert_eq!(rejected.1["field"], "pin");
}

#[tokio::test]
async fn pairing_rejects_expired_consumed_and_locked_pins_with_distinct_codes() {
    let app = app().await;
    let cookie = create_identity(&app.router, "Alice").await;
    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&cookie)),
    )
    .await;
    let pin = minted.1["pin"]
        .as_str()
        .expect("pin is a string")
        .to_string();

    let consumed = pair(&app.router, "Alice", &pin).await;
    assert_eq!(consumed.0, StatusCode::OK);
    let again = pair(&app.router, "Alice", &pin).await;
    assert_eq!(again.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(again.1["code"], "pairing_pin_consumed");
    assert_eq!(again.1["field"], "pin");

    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&cookie)),
    )
    .await;
    let expired_pin = minted.1["pin"]
        .as_str()
        .expect("pin is a string")
        .to_string();
    let mut connection = sqlite_connection(&app.db).await;
    sqlx::query("UPDATE pairing_pins SET expires_at = unixepoch() - 1 WHERE pin = ?")
        .bind(&expired_pin)
        .execute(&mut connection)
        .await
        .expect("pin expires");
    connection.close().await.expect("connection closes");
    let expired = pair(&app.router, "Alice", &expired_pin).await;
    assert_eq!(expired.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(expired.1["code"], "pairing_pin_expired");
    assert_eq!(expired.1["field"], "pin");

    let minted = send(
        &app.router,
        request("POST", "/api/identity/pin", "", Some(&cookie)),
    )
    .await;
    let locked_pin = minted.1["pin"]
        .as_str()
        .expect("pin is a string")
        .to_string();
    create_identity(&app.router, "Bob").await;
    for _ in 0..5 {
        let attempt = pair(&app.router, "Bob", &locked_pin).await;
        assert_eq!(attempt.0, StatusCode::UNPROCESSABLE_ENTITY, "{}", attempt.1);
        assert_eq!(attempt.1["code"], "pairing_pin_wrong");
    }
    let locked = pair(&app.router, "Alice", &locked_pin).await;
    assert_eq!(locked.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(locked.1["code"], "pairing_pin_locked");
    assert_eq!(locked.1["field"], "pin");
}

async fn pair(router: &Router, username: &str, pin: &str) -> TestResponse {
    send(
        router,
        request(
            "POST",
            "/api/identity/pair",
            &json!({"username": username, "pin": pin}).to_string(),
            None,
        ),
    )
    .await
}

async fn create_identity(router: &Router, username: &str) -> String {
    let created = send(
        router,
        request(
            "PUT",
            "/api/identity",
            &json!({"username": username}).to_string(),
            None,
        ),
    )
    .await;
    assert_eq!(created.0, StatusCode::OK, "{}", created.1);
    cookie_pair(&created.2.expect("identity cookie is set"))
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

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

fn cookie_pair_value(cookie: &str) -> String {
    cookie
        .split_once('=')
        .expect("cookie has a value")
        .1
        .to_string()
}

struct TestApp {
    router: Router,
    storage: Storage,
    db: std::path::PathBuf,
    _repo: TempDir,
}

async fn app() -> TestApp {
    let repo = tempfile::tempdir().expect("project creates");
    fs::create_dir_all(repo.path().join(".tasks/core")).expect("task directory creates");
    let db = repo.path().join("pairing-api-test.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let state = AppState::single(repo.path().to_path_buf(), storage.clone()).expect("state builds");
    let router = router(state);
    TestApp {
        router,
        storage,
        db,
        _repo: repo,
    }
}

async fn sqlite_connection(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection opens")
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs() as i64
}
