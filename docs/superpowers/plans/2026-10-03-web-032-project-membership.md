# Project Membership (WEB-032) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Require per-project membership (owner or editor) for writes and server admin for project registration in `filer-task-web`, with a migration that keeps existing users' access.

**Architecture:** Migration 0007 adds `users.is_admin`, `project_members`, and `access_requests`, and backfills them from the activity log. Three Axum extractors (`ServerAdmin`, `ProjectEditor`, `ProjectOwner`) in `src/access.rs` replace `Actor` on write handlers and reject before body parsing. New member routes, CLI subcommands, and browser controls sit on a membership storage module.

**Tech Stack:** Rust 2024, Axum 0.8.9, SQLx 0.9 (SQLite), Tokio; Preact + htm frontend with Node's built-in test runner.

**Spec:** `docs/superpowers/specs/2026-10-03-web-032-project-membership-design.md`

## Global Constraints

- Every `cargo` command runs from Git Bash in `D:/Project/Filer` with `+stable-x86_64-pc-windows-gnu` and `--target x86_64-pc-windows-gnu`. MSVC `link.exe` is not installed, and `.cargo/config` pins the MSVC target, so both flags are required.
- JS tests run inside `cargo test` through `tests/frontend_js_test.rs`; Node.js 21 or later must be on `PATH`.
- Migration file: `tools/filer-task-web/migrations/0007_project_membership.sql`. Schema version after it: `7`.
- Roles are stored as the TEXT values `'owner'` and `'editor'`.
- Error codes, exactly: `identity_required` (401), `admin_required` (403), `project_role_required` (403), `last_owner` (409), `already_member` (409), `access_request_not_found` (404), `user_not_found` (404).
- Activity actions, exactly: `member.grant`, `member.role`, `member.revoke`, `access.request`, `access.deny`. Detail for grant and role change: `"<username> as <role>"`; for revoke and deny: `"<username>"`; for request: none.
- JSON fields, exactly: `my_role` on each `GET /api/projects` entry (`"owner"`, `"editor"`, `"pending"`, or `null`) and `is_admin` (boolean) on identity responses.
- CLI usage lines, exactly:
  ```
  filer-task-web admin-grant <username> [--database <path>]
  filer-task-web admin-revoke <username> [--database <path>]
  filer-task-web member-grant <project> <username> --role owner|editor [--database <path>]
  filer-task-web member-revoke <project> <username> [--database <path>]
  filer-task-web member-list <project> [--database <path>]
  ```
- Startup warning text for a non-loopback host: `warning: listening on <host>: reads are open to anyone who can reach this address; writes need project membership`.
- Documentation follows `docs/WRITING_GUIDE.md`: present tense, no bold or italic in running prose, no "now", "new", or "legacy" in reference docs.
- Commits use the repository's conventional style, scoped `task-web` or `web`.

## Review Focus

1. A project whose name needs percent-encoding (a space) must be guarded by its decoded name, exactly as the registry knows it. Test in Task 4.
2. A member revoked while their browser is open must be refused on their next write, and the browser must drop its write controls. Tests in Task 5 (server) and Task 8 (client hook).
3. Two owners demoting each other at the same moment must leave exactly one owner, never zero. Test in Task 3.
4. A renamed member must appear under their current name in the member list while activity keeps the old name. Test in Task 5.
5. Removing a registration and registering the same project again must not resurrect old members. Test in Task 5.

---

### Task 0: Start WEB-032 and align the spec with the codebase

Reading the code for this plan surfaced facts the spec did not know: the UI has no control for removing a registration, the palette's only write is creating a project, identity responses other than GET also need `is_admin`, and a test closes storage and still expects the project list. This task records those facts in the spec before any code depends on them.

**Files:**
- Modify: `docs/superpowers/specs/2026-10-03-web-032-project-membership-design.md`
- Modify (by command): `.tasks/web/WEB-032-require-project-membership-for-writes.md`

- [ ] **Step 1: Start the task**

Run: `cargo +stable-x86_64-pc-windows-gnu run -q --locked -p taskroot --target x86_64-pc-windows-gnu -- start web:WEB-032`
Expected: output starting with `Task Started` and `Task: web:WEB-032`. If it refuses because the parent epic `WEB-014` is `Deferred`, stop and ask the user whether to move WEB-014 to `In Progress` first.

- [ ] **Step 2: Edit the spec's HTTP API section**

Replace:
```
Existing responses gain two fields. `GET /api/projects` adds `my_role` to each summary: `owner`, `editor`, `pending`, or `null`. The route stays open and fills the field only when the request carries a valid session. `GET /api/identity` adds `is_admin`. These fields let the UI decide which controls to show without extra requests.
```
with:
```
Existing responses gain two fields. `GET /api/projects` adds `my_role` to each summary: `owner`, `editor`, `pending`, or `null`. The route stays open and fills the field only when the request carries a valid session. If the session or role lookup fails, the list still returns with every `my_role` null and the server logs the error, because reads stay open. `GET /api/identity`, `PUT /api/identity`, and `POST /api/identity/pair` add `is_admin`. These fields let the UI decide which controls to show without extra requests.

Revoking a user who is not a member succeeds without change, and requesting or withdrawing access answers `204 No Content`.
```

- [ ] **Step 3: Edit the spec's Browser UI section**

Replace:
```
When the user cannot write to the open project, the UI hides the New task sidebar item, the transition buttons in `DrawerActions`, the form in `DrawerEdit`, the checkboxes in `DrawerCriteria`, and the write commands in the command palette.
```
with:
```
When the user cannot write to the open project, the UI hides the New task sidebar item, the transition buttons in `DrawerActions`, and the Edit button that opens `DrawerEdit`, and disables the checkboxes in `DrawerCriteria` so criteria stay readable. The command palette offers its create-a-project row only to admins.
```

Replace:
```
Only admins see the project open and create dialogs. Other users see "Only server admins can register projects." Only owners see the action that removes a registration.
```
with:
```
Only admins see the project open and create buttons. Other users see "Only server admins can register projects." Only owners see the domain, task type, and tag editors; other users see "Only project owners can change domains, task types, and tags." The UI has no control for removing a registration; the route requires the owner role.
```

Replace:
```
The Activity screen renders the membership actions as readable sentences, such as "approved Minh as editor".
```
with:
```
The Activity screen shows a readable label for each membership action, such as "granted access" beside the detail "Minh as editor".
```

- [ ] **Step 4: Edit the spec's Testing section**

Replace:
```
CLI tests use the shared runner in `tests/cli`: grant and revoke for admins and members, `member-list` output, the last-owner warning, and the startup warning appearing on stderr for a non-loopback host and not for loopback.
```
with:
```
CLI tests use the shared runner in `tests/cli`: grant and revoke for admins and members, `member-list` output, and the last-owner warning. The startup warning text comes from `access::exposure_warning`, which tests call directly for loopback and non-loopback addresses, because binding a non-loopback address in a test can trigger a firewall prompt. A serve test confirms the loopback default prints no warning.
```

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-10-03-web-032-project-membership-design.md .tasks/web/WEB-032-require-project-membership-for-writes.md
git commit -m "task(web): start WEB-032 and align its spec with the codebase"
```

---

### Task 1: Migration 0007 with backfill

**Files:**
- Create: `tools/filer-task-web/migrations/0007_project_membership.sql`
- Create: `tools/filer-task-web/tests/membership_storage_test.rs`
- Modify: `tools/filer-task-web/src/storage/mod.rs:15`
- Modify: `tools/filer-task-web/tests/storage_test.rs:14,27,44`
- Modify: `tools/filer-task-web/tests/sessions_storage_test.rs:115`

**Interfaces:**
- Consumes: migrations 0001 to 0006; tables `users`, `project_registrations`, `activity`.
- Produces: table `project_members(project TEXT, user_id INTEGER, role TEXT, granted_by INTEGER NULL, granted_at INTEGER)`, table `access_requests(project TEXT, user_id INTEGER, requested_at INTEGER)`, column `users.is_admin INTEGER` (0 or 1). Test helpers `stage_schema_six`, `sqlite_connection`, `member_rows`, `admin_flags` in `tests/membership_storage_test.rs`, reused by Tasks 2 and 3.

- [ ] **Step 1: Write the failing backfill tests**

Create `tools/filer-task-web/tests/membership_storage_test.rs`:

```rust
//! Exercises membership storage: the 0007 backfill, the first-identity admin
//! rule, and the invariants that keep every project with an owner.

use std::{fs, path::Path};

use sqlx::{Connection, SqliteConnection, migrate::Migrator, sqlite::SqliteConnectOptions};
use tempfile::TempDir;

use filer_task_web::storage::Storage;

#[tokio::test]
async fn migration_backfills_the_admin_owners_and_editors() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    stage_schema_six(&temp, &db).await;
    let mut connection = sqlite_connection(&db).await;
    for (name, key) in [("Alice", "alice"), ("Bob", "bob"), ("Cara", "cara")] {
        sqlx::query("INSERT INTO users (display_name, name_key) VALUES (?, ?)")
            .bind(name)
            .bind(key)
            .execute(&mut connection)
            .await
            .expect("user inserts");
    }
    for name in ["alpha", "beta", "gamma"] {
        sqlx::query("INSERT INTO project_registrations (name, root) VALUES (?, ?)")
            .bind(name)
            .bind(name.as_bytes())
            .execute(&mut connection)
            .await
            .expect("registration inserts");
    }
    // Inserted in order, so activity ids ascend down this list. Alpha was
    // registered twice; the later registration by Bob decides its owner.
    for (user_id, username, project, action) in [
        (1, "Alice", "alpha", "project.register"),
        (3, "Cara", "alpha", "task.create"),
        (2, "Bob", "alpha", "project.register"),
        (2, "Bob", "beta", "task.edit"),
        (3, "Cara", "ghost", "project.register"),
    ] {
        sqlx::query("INSERT INTO activity (user_id, username, project, action) VALUES (?, ?, ?, ?)")
            .bind(user_id)
            .bind(username)
            .bind(project)
            .bind(action)
            .execute(&mut connection)
            .await
            .expect("activity inserts");
    }
    connection.close().await.expect("connection closes");

    let storage = Storage::open(&db).await.expect("storage migrates");
    assert_eq!(storage.schema_version().await.expect("version reads"), 7);
    storage.close().await;

    assert_eq!(
        admin_flags(&db).await,
        vec![
            ("Alice".to_string(), true),
            ("Bob".to_string(), false),
            ("Cara".to_string(), false),
        ]
    );
    assert_eq!(
        member_rows(&db, "alpha").await,
        vec![
            ("Bob".to_string(), "owner".to_string()),
            ("Alice".to_string(), "editor".to_string()),
            ("Cara".to_string(), "editor".to_string()),
        ]
    );
    assert_eq!(
        member_rows(&db, "beta").await,
        vec![
            ("Alice".to_string(), "owner".to_string()),
            ("Bob".to_string(), "editor".to_string()),
        ]
    );
    assert_eq!(
        member_rows(&db, "gamma").await,
        vec![("Alice".to_string(), "owner".to_string())]
    );
    assert!(member_rows(&db, "ghost").await.is_empty());
}

#[tokio::test]
async fn migration_leaves_projects_ownerless_without_users() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    stage_schema_six(&temp, &db).await;
    let mut connection = sqlite_connection(&db).await;
    sqlx::query("INSERT INTO project_registrations (name, root) VALUES ('alpha', X'61')")
        .execute(&mut connection)
        .await
        .expect("registration inserts");
    connection.close().await.expect("connection closes");

    let storage = Storage::open(&db).await.expect("storage migrates");
    storage.close().await;

    assert!(member_rows(&db, "alpha").await.is_empty());
}

async fn stage_schema_six(temp: &TempDir, db: &Path) {
    let staged = temp.path().join("staged-migrations");
    fs::create_dir_all(&staged).expect("staged migrations dir creates");
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "0001_storage_foundation.sql",
        "0002_project_registrations.sql",
        "0003_users.sql",
        "0004_activity.sql",
        "0005_sessions.sql",
        "0006_session_device_labels.sql",
    ] {
        fs::copy(crate_dir.join("migrations").join(name), staged.join(name))
            .expect("migration file copies");
    }
    let migrator = Migrator::new(staged).await.expect("staged migrator loads");
    let mut connection = sqlite_connection(db).await;
    migrator
        .run(&mut connection)
        .await
        .expect("schema six applies");
    connection.close().await.expect("connection closes");
}

async fn sqlite_connection(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection opens")
}

/// Members of `project` as (username, role), owners first, then by name.
async fn member_rows(db: &Path, project: &str) -> Vec<(String, String)> {
    let mut connection = sqlite_connection(db).await;
    let rows = sqlx::query_as(
        "SELECT u.display_name, m.role FROM project_members m \
         JOIN users u ON u.id = m.user_id WHERE m.project = ? \
         ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, u.name_key",
    )
    .bind(project)
    .fetch_all(&mut connection)
    .await
    .expect("members read");
    connection.close().await.expect("connection closes");
    rows
}

async fn admin_flags(db: &Path) -> Vec<(String, bool)> {
    let mut connection = sqlite_connection(db).await;
    let rows = sqlx::query_as("SELECT display_name, is_admin FROM users ORDER BY id")
        .fetch_all(&mut connection)
        .await
        .expect("admin flags read");
    connection.close().await.expect("connection closes");
    rows
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_storage_test`
Expected: FAIL. `migration_backfills_the_admin_owners_and_editors` fails on `left: 6, right: 7`.

- [ ] **Step 3: Write the migration**

Create `tools/filer-task-web/migrations/0007_project_membership.sql`:

```sql
-- Per-project membership, pending access requests, and a server admin flag.
-- The backfill keeps write access for everyone who has already used a project:
-- the first user becomes admin, each project's latest registrant becomes its
-- owner (the admin when no registration was recorded), and every other user
-- with activity in a project becomes an editor there.

ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0, 1));

CREATE TABLE project_members (
    project    TEXT    NOT NULL REFERENCES project_registrations(name) ON DELETE CASCADE,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    role       TEXT    NOT NULL CHECK (role IN ('owner', 'editor')),
    granted_by INTEGER REFERENCES users(id),
    granted_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (project, user_id)
) STRICT;

CREATE INDEX idx_project_members_user ON project_members(user_id);

CREATE TABLE access_requests (
    project      TEXT    NOT NULL REFERENCES project_registrations(name) ON DELETE CASCADE,
    user_id      INTEGER NOT NULL REFERENCES users(id),
    requested_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (project, user_id)
) STRICT;

CREATE INDEX idx_access_requests_user ON access_requests(user_id);

UPDATE users SET is_admin = 1 WHERE id = (SELECT MIN(id) FROM users);

INSERT INTO project_members (project, user_id, role)
SELECT a.project, a.user_id, 'owner'
FROM activity a
JOIN project_registrations p ON p.name = a.project
WHERE a.id = (
    SELECT MAX(latest.id)
    FROM activity latest
    JOIN users u ON u.id = latest.user_id
    WHERE latest.action = 'project.register' AND latest.project = a.project
);

INSERT INTO project_members (project, user_id, role)
SELECT p.name, (SELECT MIN(id) FROM users), 'owner'
FROM project_registrations p
WHERE EXISTS (SELECT 1 FROM users)
  AND NOT EXISTS (SELECT 1 FROM project_members m WHERE m.project = p.name);

INSERT OR IGNORE INTO project_members (project, user_id, role)
SELECT DISTINCT a.project, a.user_id, 'editor'
FROM activity a
JOIN project_registrations p ON p.name = a.project
JOIN users u ON u.id = a.user_id;
```

- [ ] **Step 4: Bump the schema version assertions from 6 to 7**

In `tools/filer-task-web/src/storage/mod.rs` line 15, change `//! assert_eq!(storage.schema_version().await?, 6);` to `//! assert_eq!(storage.schema_version().await?, 7);`.

In `tools/filer-task-web/tests/storage_test.rs` lines 14, 27, and 44, and `tools/filer-task-web/tests/sessions_storage_test.rs` line 115, change the expected value `6` to `7` in each `assert_eq!(... schema_version() ..., 6)`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_storage_test --test storage_test --test sessions_storage_test`
Expected: PASS, all tests.

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --doc`
Expected: PASS (the storage doctest asserts version 7).

- [ ] **Step 6: Commit**

```bash
git add tools/filer-task-web/migrations/0007_project_membership.sql tools/filer-task-web/tests/membership_storage_test.rs tools/filer-task-web/src/storage/mod.rs tools/filer-task-web/tests/storage_test.rs tools/filer-task-web/tests/sessions_storage_test.rs
git commit -m "feat(task-web): add project membership schema with backfill"
```

---

### Task 2: First identity becomes admin

**Files:**
- Modify: `tools/filer-task-web/src/storage/identities.rs` (`ResolvedSession`, `create_identity`, `resolve_identity`, `insert_user`, `decode_resolved`; add `is_admin`, `admin_count`)
- Test: `tools/filer-task-web/tests/membership_storage_test.rs`

**Interfaces:**
- Consumes: column `users.is_admin` from Task 1.
- Produces: `ResolvedSession { identity, session_id, last_seen, is_admin: bool }`; `Storage::is_admin(&self, user_id: i64) -> Result<bool, StorageError>`; `Storage::admin_count(&self) -> Result<i64, StorageError>`. `create_identity` and `mint_recovery_session` make the first user on an empty database admin.

- [ ] **Step 1: Write the failing tests**

Append to `tools/filer-task-web/tests/membership_storage_test.rs`:

```rust
#[tokio::test]
async fn the_first_identity_on_an_empty_database_becomes_admin() {
    let (storage, _temp) = fresh_storage().await;

    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("first identity creates");
    let bob = storage
        .create_identity("Bob", "Test browser")
        .await
        .expect("second identity creates");

    assert!(storage.is_admin(alice.identity.user_id).await.expect("flag reads"));
    assert!(!storage.is_admin(bob.identity.user_id).await.expect("flag reads"));
    let resolved = storage
        .resolve_identity(&alice.session_token)
        .await
        .expect("lookup succeeds")
        .expect("session exists");
    assert!(resolved.is_admin);
    storage.close().await;
}

#[tokio::test]
async fn concurrent_first_identities_produce_exactly_one_admin() {
    let (storage, _temp) = fresh_storage().await;

    let (first, second) = tokio::join!(
        storage.create_identity("Alice", "Browser A"),
        storage.create_identity("Bob", "Browser B"),
    );
    first.expect("first identity creates");
    second.expect("second identity creates");

    assert_eq!(storage.admin_count().await.expect("admins count"), 1);
    storage.close().await;
}

#[tokio::test]
async fn a_recovery_session_on_an_empty_database_creates_an_admin() {
    let (storage, _temp) = fresh_storage().await;

    let minted = storage
        .mint_recovery_session("Alice", "Recovery CLI")
        .await
        .expect("recovery session mints");

    assert!(storage.is_admin(minted.identity.user_id).await.expect("flag reads"));
    storage.close().await;
}

async fn fresh_storage() -> (Storage, TempDir) {
    let temp = tempfile::tempdir().expect("temp dir created");
    let storage = Storage::open(temp.path().join("state.sqlite3"))
        .await
        .expect("storage opens");
    (storage, temp)
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_storage_test`
Expected: FAIL to compile with `no method named `is_admin` found for struct `Storage``.

- [ ] **Step 3: Implement the admin rule**

In `tools/filer-task-web/src/storage/identities.rs`:

Add the field to `ResolvedSession`:
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSession {
    pub identity: StoredIdentity,
    pub session_id: i64,
    pub last_seen: i64,
    pub is_admin: bool,
}
```

In `create_identity`, replace the insert statement text:
```rust
        let row = sqlx::query(
            "INSERT INTO users (display_name, name_key, is_admin) \
             VALUES (?, ?, NOT EXISTS (SELECT 1 FROM users)) \
             ON CONFLICT(name_key) DO NOTHING \
             RETURNING id, display_name",
        )
```

In `insert_user`, replace the insert statement text the same way:
```rust
    let row = sqlx::query(
        "INSERT INTO users (display_name, name_key, is_admin) \
         VALUES (?, ?, NOT EXISTS (SELECT 1 FROM users)) \
         ON CONFLICT(name_key) DO NOTHING \
         RETURNING id, display_name",
    )
```

In `resolve_identity`, replace the select statement text:
```rust
        let row = sqlx::query(
            "SELECT s.id AS session_id, s.last_seen AS last_seen, u.id AS user_id, \
                    u.display_name, u.is_admin AS is_admin \
             FROM sessions s JOIN users u ON u.id = s.user_id \
             WHERE s.token = ?",
        )
```

In `decode_resolved`, read the flag and return it:
```rust
    let is_admin = row
        .try_get("is_admin")
        .map_err(|source| StorageError::Operation { operation, source })?;
    Ok(ResolvedSession {
        identity: StoredIdentity { user_id, username },
        session_id,
        last_seen,
        is_admin,
    })
```

Add these methods inside the existing `impl Storage` block, after `revoke_session`:
```rust
    /// Whether `user_id` is a server admin. A missing user is not an admin.
    pub async fn is_admin(&self, user_id: i64) -> Result<bool, StorageError> {
        let flag: Option<bool> = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "read admin flag",
                source,
            })?;
        Ok(flag.unwrap_or(false))
    }

    pub async fn admin_count(&self) -> Result<i64, StorageError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "count admins",
                source,
            })
    }
```

The insert takes SQLite's write lock before it evaluates `NOT EXISTS`, so a second concurrent insert waits for the first to commit and then sees a user. Add this comment above the `create_identity` insert:
```rust
        // The insert holds SQLite's write lock while it evaluates NOT EXISTS,
        // so of two identities created at once on an empty database exactly
        // one becomes admin.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_storage_test --test sessions_storage_test --test identity_storage_test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/filer-task-web/src/storage/identities.rs tools/filer-task-web/tests/membership_storage_test.rs
git commit -m "feat(task-web): make the first identity a server admin"
```

---

### Task 3: Membership storage

**Files:**
- Create: `tools/filer-task-web/src/storage/membership.rs`
- Modify: `tools/filer-task-web/src/storage/mod.rs` (module, re-exports, three `StorageError` variants)
- Modify: `tools/filer-task-web/src/storage/projects.rs` (add `insert_project_registration_with_owner`)
- Modify: `tools/filer-task-web/src/storage/identities.rs` (make `decode_identity` `pub(super)`)
- Test: `tools/filer-task-web/tests/membership_storage_test.rs`

**Interfaces:**
- Consumes: `Storage`, `StorageError`, `ProjectRegistration`, tables from Task 1.
- Produces (all re-exported from `filer_task_web::storage`):
  - `enum Role { Owner, Editor }` with `as_str(self) -> &'static str`, `parse(&str) -> Option<Role>`, `satisfies(self, required: Role) -> bool`, `Display`, serde lowercase.
  - `enum ProjectAccess { Owner, Editor, Pending }`, serde lowercase, `From<Role>`.
  - `struct MemberRecord { user_id: i64, username: String, role: Role, granted_by: Option<String>, granted_at: i64 }`
  - `struct AccessRequestRecord { user_id: i64, username: String, requested_at: i64 }`
  - `struct GrantOutcome { username: String, previous: Option<Role> }`
  - `struct RemovedMember { username: String, role: Role }`
  - `Storage::project_role(&self, project: &str, user_id: i64) -> Result<Option<Role>, StorageError>`
  - `Storage::user_project_access(&self, user_id: i64) -> Result<HashMap<String, ProjectAccess>, StorageError>`
  - `Storage::list_members(&self, project: &str) -> Result<Vec<MemberRecord>, StorageError>`
  - `Storage::list_access_requests(&self, project: &str) -> Result<Vec<AccessRequestRecord>, StorageError>`
  - `Storage::grant_member(&self, project: &str, user_id: i64, role: Role, granted_by: Option<i64>, protect_last_owner: bool) -> Result<GrantOutcome, StorageError>`
  - `Storage::remove_member(&self, project: &str, user_id: i64, protect_last_owner: bool) -> Result<Option<RemovedMember>, StorageError>`
  - `Storage::request_access(&self, project: &str, user_id: i64) -> Result<bool, StorageError>` (true when a request was created)
  - `Storage::delete_access_request(&self, project: &str, user_id: i64) -> Result<Option<String>, StorageError>` (the requester's username when one existed)
  - `Storage::insert_project_registration_with_owner(&self, registration: &ProjectRegistration, owner_user_id: i64) -> Result<(), StorageError>`
  - `StorageError::LastOwner`, `StorageError::AlreadyMember`, `StorageError::ProjectNotRegistered(String)`

- [ ] **Step 1: Write the failing tests**

Change the `use filer_task_web::storage::Storage;` line at the top of `tools/filer-task-web/tests/membership_storage_test.rs` to:
```rust
use filer_task_web::storage::{
    GrantOutcome, ProjectAccess, ProjectRegistration, RemovedMember, Role, Storage, StorageError,
};
```

Append:
```rust
#[tokio::test]
async fn registering_inserts_the_owner_in_the_same_transaction() {
    let (storage, temp) = fresh_storage().await;
    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("identity creates")
        .identity
        .user_id;

    storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("alpha", temp.path().join("alpha")),
            alice,
        )
        .await
        .expect("registration commits");
    assert_eq!(
        storage.project_role("alpha", alice).await.expect("role reads"),
        Some(Role::Owner)
    );

    let error = storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("beta", temp.path().join("beta")),
            9_999,
        )
        .await
        .expect_err("an unknown owner is refused");
    assert!(matches!(error, StorageError::Operation { .. }), "{error}");
    let names: Vec<_> = storage
        .project_registrations()
        .await
        .expect("registrations load")
        .into_iter()
        .map(|registration| registration.name)
        .collect();
    assert_eq!(names, vec!["alpha"], "the failed owner insert rolled back beta");
    storage.close().await;
}

#[tokio::test]
async fn approving_a_request_grants_the_role_and_deletes_the_request() {
    let (storage, temp) = fresh_storage().await;
    let (alice, bob) = owner_and_user(&storage, &temp).await;

    assert!(storage.request_access("alpha", bob).await.expect("request records"));
    assert!(!storage.request_access("alpha", bob).await.expect("repeat succeeds"));
    assert_eq!(
        storage.user_project_access(bob).await.expect("access reads").get("alpha"),
        Some(&ProjectAccess::Pending)
    );
    assert_eq!(
        storage.list_access_requests("alpha").await.expect("requests list")[0].username,
        "Bob"
    );

    let outcome = storage
        .grant_member("alpha", bob, Role::Editor, Some(alice), true)
        .await
        .expect("grant succeeds");

    assert_eq!(
        outcome,
        GrantOutcome {
            username: "Bob".to_string(),
            previous: None
        }
    );
    assert!(storage.list_access_requests("alpha").await.expect("requests list").is_empty());
    assert_eq!(
        storage.user_project_access(bob).await.expect("access reads").get("alpha"),
        Some(&ProjectAccess::Editor)
    );
    let members = storage.list_members("alpha").await.expect("members list");
    assert_eq!(members[1].username, "Bob");
    assert_eq!(members[1].granted_by.as_deref(), Some("Alice"));
    let error = storage
        .request_access("alpha", bob)
        .await
        .expect_err("a member cannot request access");
    assert!(matches!(error, StorageError::AlreadyMember), "{error}");
    storage.close().await;
}

#[tokio::test]
async fn the_last_owner_cannot_be_removed_or_demoted_except_by_override() {
    let (storage, temp) = fresh_storage().await;
    let (alice, bob) = owner_and_user(&storage, &temp).await;
    storage
        .grant_member("alpha", bob, Role::Editor, Some(alice), true)
        .await
        .expect("bob joins");

    let demote = storage
        .grant_member("alpha", alice, Role::Editor, None, true)
        .await
        .expect_err("the last owner cannot be demoted");
    assert!(matches!(demote, StorageError::LastOwner), "{demote}");
    let remove = storage
        .remove_member("alpha", alice, true)
        .await
        .expect_err("the last owner cannot be removed");
    assert!(matches!(remove, StorageError::LastOwner), "{remove}");
    assert_eq!(
        storage.project_role("alpha", alice).await.expect("role reads"),
        Some(Role::Owner)
    );

    let promoted = storage
        .grant_member("alpha", bob, Role::Owner, Some(alice), true)
        .await
        .expect("bob is promoted");
    assert_eq!(promoted.previous, Some(Role::Editor));
    let demoted = storage
        .grant_member("alpha", alice, Role::Editor, Some(bob), true)
        .await
        .expect("alice steps down while bob owns");
    assert_eq!(demoted.previous, Some(Role::Owner));

    let refused = storage
        .remove_member("alpha", bob, true)
        .await
        .expect_err("bob is the last owner");
    assert!(matches!(refused, StorageError::LastOwner), "{refused}");
    let removed = storage
        .remove_member("alpha", bob, false)
        .await
        .expect("the override removes the last owner");
    assert_eq!(
        removed,
        Some(RemovedMember {
            username: "Bob".to_string(),
            role: Role::Owner
        })
    );
    assert!(
        storage
            .list_members("alpha")
            .await
            .expect("members list")
            .iter()
            .all(|member| member.role != Role::Owner)
    );
    assert_eq!(
        storage.remove_member("alpha", bob, true).await.expect("a non-member removes"),
        None
    );
    storage.close().await;
}

#[tokio::test]
async fn two_owners_demoting_each_other_at_once_leave_one_owner() {
    let (storage, temp) = fresh_storage().await;
    let (alice, bob) = owner_and_user(&storage, &temp).await;
    storage
        .grant_member("alpha", bob, Role::Owner, Some(alice), true)
        .await
        .expect("bob is an owner");

    let (first, second) = tokio::join!(
        storage.grant_member("alpha", alice, Role::Editor, Some(bob), true),
        storage.grant_member("alpha", bob, Role::Editor, Some(alice), true),
    );

    assert_eq!(
        [first.is_ok(), second.is_ok()].iter().filter(|ok| **ok).count(),
        1,
        "first: {first:?}, second: {second:?}"
    );
    let refused = if first.is_err() { first } else { second };
    assert!(matches!(refused, Err(StorageError::LastOwner)), "{refused:?}");
    let owners = storage
        .list_members("alpha")
        .await
        .expect("members list")
        .into_iter()
        .filter(|member| member.role == Role::Owner)
        .count();
    assert_eq!(owners, 1);
    storage.close().await;
}

#[tokio::test]
async fn grants_name_the_missing_user_or_project() {
    let (storage, temp) = fresh_storage().await;
    let (alice, _bob) = owner_and_user(&storage, &temp).await;

    let missing_user = storage
        .grant_member("alpha", 9_999, Role::Editor, Some(alice), true)
        .await
        .expect_err("unknown user is refused");
    assert!(matches!(missing_user, StorageError::IdentityNotFound(9_999)), "{missing_user}");
    let missing_project = storage
        .grant_member("ghost", alice, Role::Editor, None, true)
        .await
        .expect_err("unknown project is refused");
    assert!(
        matches!(&missing_project, StorageError::ProjectNotRegistered(name) if name == "ghost"),
        "{missing_project}"
    );
    storage.close().await;
}

#[tokio::test]
async fn removing_a_registration_deletes_its_members_and_requests() {
    let (storage, temp) = fresh_storage().await;
    let (_alice, bob) = owner_and_user(&storage, &temp).await;
    storage.request_access("alpha", bob).await.expect("request records");

    assert!(
        storage
            .delete_project_registration("alpha")
            .await
            .expect("registration deletes")
    );

    assert!(storage.list_members("alpha").await.expect("members list").is_empty());
    assert!(storage.list_access_requests("alpha").await.expect("requests list").is_empty());
    assert!(storage.user_project_access(bob).await.expect("access reads").is_empty());
    storage.close().await;
}

#[tokio::test]
async fn deleting_a_request_reports_who_asked() {
    let (storage, temp) = fresh_storage().await;
    let (_alice, bob) = owner_and_user(&storage, &temp).await;
    storage.request_access("alpha", bob).await.expect("request records");

    assert_eq!(
        storage.delete_access_request("alpha", bob).await.expect("request deletes"),
        Some("Bob".to_string())
    );
    assert_eq!(
        storage.delete_access_request("alpha", bob).await.expect("repeat deletes"),
        None
    );
    storage.close().await;
}

/// Alice owns a registered project named alpha; Bob exists with no access.
async fn owner_and_user(storage: &Storage, temp: &TempDir) -> (i64, i64) {
    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("alice creates")
        .identity
        .user_id;
    let bob = storage
        .create_identity("Bob", "Test browser")
        .await
        .expect("bob creates")
        .identity
        .user_id;
    storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("alpha", temp.path().join("alpha")),
            alice,
        )
        .await
        .expect("alpha registers");
    (alice, bob)
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_storage_test`
Expected: FAIL to compile with `unresolved imports` for `GrantOutcome`, `ProjectAccess`, `RemovedMember`, `Role`.

- [ ] **Step 3: Add the storage errors**

In `tools/filer-task-web/src/storage/mod.rs`, add three variants to `StorageError` after `IdentityNotFound(i64),`:
```rust
    LastOwner,
    AlreadyMember,
    ProjectNotRegistered(String),
```

Add their arms to `impl fmt::Display for StorageError`, after the `IdentityNotFound` arm:
```rust
            Self::LastOwner => write!(formatter, "a project must keep at least one owner"),
            Self::AlreadyMember => write!(formatter, "the user is already a member of the project"),
            Self::ProjectNotRegistered(name) => write!(formatter, "project {name} is not registered"),
```

Extend the `None` arm of `impl Error for StorageError` so the match stays exhaustive:
```rust
            Self::InvalidData { .. }
            | Self::Internal { .. }
            | Self::UsernameTaken
            | Self::IdentityNotFound(_)
            | Self::LastOwner
            | Self::AlreadyMember
            | Self::ProjectNotRegistered(_) => None,
```

Add the module and re-exports beside the existing ones:
```rust
mod membership;
```
```rust
pub use membership::{
    AccessRequestRecord, GrantOutcome, MemberRecord, ProjectAccess, RemovedMember, Role,
};
```

In `tools/filer-task-web/src/storage/identities.rs`, change `fn decode_identity(` to `pub(super) fn decode_identity(`.

- [ ] **Step 4: Write the membership module**

Create `tools/filer-task-web/src/storage/membership.rs`:

```rust
//! # Membership Storage
//!
//! Persists who may change each project and who is waiting for access. Members
//! and pending requests live in separate tables, so a membership check is one
//! primary-key lookup with no status filter. A check that spans rows, such as
//! a project keeping at least one owner, runs inside the transaction that would
//! break it and after that transaction's first write, which holds SQLite's
//! write lock, so two concurrent changes cannot both pass it.

use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{Storage, StorageError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Editor,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "editor" => Some(Self::Editor),
            _ => None,
        }
    }

    /// Whether holding this role grants what `required` asks for. An owner can
    /// do everything an editor can.
    pub fn satisfies(self, required: Role) -> bool {
        self == Self::Owner || self == required
    }
}

impl fmt::Display for Role {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A user's standing on one project, as the browser needs it: a role, or a
/// request that is still waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectAccess {
    Owner,
    Editor,
    Pending,
}

impl From<Role> for ProjectAccess {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Editor => Self::Editor,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRecord {
    pub user_id: i64,
    pub username: String,
    pub role: Role,
    pub granted_by: Option<String>,
    pub granted_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessRequestRecord {
    pub user_id: i64,
    pub username: String,
    pub requested_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantOutcome {
    pub username: String,
    pub previous: Option<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedMember {
    pub username: String,
    pub role: Role,
}

impl Storage {
    pub async fn project_role(
        &self,
        project: &str,
        user_id: i64,
    ) -> Result<Option<Role>, StorageError> {
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM project_members WHERE project = ? AND user_id = ?")
                .bind(project)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(failed("read project role"))?;
        role.map(|role| decode_role(&role)).transpose()
    }

    pub async fn user_project_access(
        &self,
        user_id: i64,
    ) -> Result<HashMap<String, ProjectAccess>, StorageError> {
        let rows = sqlx::query(
            "SELECT project, role FROM project_members WHERE user_id = ? \
             UNION ALL \
             SELECT project, 'pending' FROM access_requests WHERE user_id = ?",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(failed("list project access"))?;
        let mut access = HashMap::new();
        for row in rows {
            let project: String = row.try_get("project").map_err(failed("decode project access"))?;
            let role: String = row.try_get("role").map_err(failed("decode project access"))?;
            let value = match role.as_str() {
                "pending" => ProjectAccess::Pending,
                other => decode_role(other)?.into(),
            };
            access.insert(project, value);
        }
        Ok(access)
    }

    pub async fn list_members(&self, project: &str) -> Result<Vec<MemberRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT m.user_id, u.display_name AS username, m.role, \
                    g.display_name AS granted_by, m.granted_at \
             FROM project_members m \
             JOIN users u ON u.id = m.user_id \
             LEFT JOIN users g ON g.id = m.granted_by \
             WHERE m.project = ? \
             ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, u.name_key",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await
        .map_err(failed("list members"))?;
        rows.iter().map(decode_member).collect()
    }

    pub async fn list_access_requests(
        &self,
        project: &str,
    ) -> Result<Vec<AccessRequestRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT r.user_id, u.display_name AS username, r.requested_at \
             FROM access_requests r JOIN users u ON u.id = r.user_id \
             WHERE r.project = ? ORDER BY r.requested_at, r.user_id",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await
        .map_err(failed("list access requests"))?;
        rows.iter()
            .map(|row| {
                Ok(AccessRequestRecord {
                    user_id: row.try_get("user_id").map_err(failed("decode access request"))?,
                    username: row.try_get("username").map_err(failed("decode access request"))?,
                    requested_at: row
                        .try_get("requested_at")
                        .map_err(failed("decode access request"))?,
                })
            })
            .collect()
    }

    /// Adds `user_id` to `project` with `role`, or changes an existing member's
    /// role, and deletes that user's pending request. With `protect_last_owner`,
    /// a change that would leave the project without an owner is refused and
    /// rolled back; the CLI passes `false` as the operator override.
    pub async fn grant_member(
        &self,
        project: &str,
        user_id: i64,
        role: Role,
        granted_by: Option<i64>,
        protect_last_owner: bool,
    ) -> Result<GrantOutcome, StorageError> {
        let mut transaction = self.pool.begin().await.map_err(failed("begin grant member"))?;
        // First statement is a write, so the checks below read under the lock.
        sqlx::query("DELETE FROM access_requests WHERE project = ? AND user_id = ?")
            .bind(project)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(failed("clear access request"))?;
        ensure_registered(&mut transaction, project).await?;
        let username = username_of(&mut transaction, user_id).await?;
        let previous = member_role(&mut transaction, project, user_id).await?;
        sqlx::query(
            "INSERT INTO project_members (project, user_id, role, granted_by) VALUES (?, ?, ?, ?) \
             ON CONFLICT(project, user_id) DO UPDATE \
             SET role = excluded.role, granted_by = excluded.granted_by, granted_at = unixepoch() \
             WHERE project_members.role <> excluded.role",
        )
        .bind(project)
        .bind(user_id)
        .bind(role.as_str())
        .bind(granted_by)
        .execute(&mut *transaction)
        .await
        .map_err(failed("upsert member"))?;
        if protect_last_owner && owner_count(&mut transaction, project).await? == 0 {
            return Err(StorageError::LastOwner);
        }
        transaction.commit().await.map_err(failed("commit grant member"))?;
        Ok(GrantOutcome { username, previous })
    }

    /// Removes `user_id` from `project`. Returns `None` when they were not a
    /// member. With `protect_last_owner`, removing the last owner is refused.
    pub async fn remove_member(
        &self,
        project: &str,
        user_id: i64,
        protect_last_owner: bool,
    ) -> Result<Option<RemovedMember>, StorageError> {
        let mut transaction = self.pool.begin().await.map_err(failed("begin remove member"))?;
        let removed: Option<String> = sqlx::query_scalar(
            "DELETE FROM project_members WHERE project = ? AND user_id = ? RETURNING role",
        )
        .bind(project)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(failed("remove member"))?;
        let Some(role) = removed else {
            return Ok(None);
        };
        let role = decode_role(&role)?;
        if protect_last_owner
            && role == Role::Owner
            && owner_count(&mut transaction, project).await? == 0
        {
            return Err(StorageError::LastOwner);
        }
        let username = username_of(&mut transaction, user_id).await?;
        transaction.commit().await.map_err(failed("commit remove member"))?;
        Ok(Some(RemovedMember { username, role }))
    }

    /// Records a pending request. Returns whether a request was created; asking
    /// again while one is pending succeeds without change.
    pub async fn request_access(&self, project: &str, user_id: i64) -> Result<bool, StorageError> {
        let mut transaction = self.pool.begin().await.map_err(failed("begin request access"))?;
        let inserted = sqlx::query(
            "INSERT INTO access_requests (project, user_id) VALUES (?, ?) \
             ON CONFLICT(project, user_id) DO NOTHING",
        )
        .bind(project)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(failed("request access"))?;
        if member_role(&mut transaction, project, user_id).await?.is_some() {
            return Err(StorageError::AlreadyMember);
        }
        transaction.commit().await.map_err(failed("commit request access"))?;
        Ok(inserted.rows_affected() == 1)
    }

    /// Deletes a pending request, for both a withdrawal and a denial. Returns
    /// the requester's username when a request existed.
    pub async fn delete_access_request(
        &self,
        project: &str,
        user_id: i64,
    ) -> Result<Option<String>, StorageError> {
        let mut transaction = self.pool.begin().await.map_err(failed("begin delete request"))?;
        let deleted: Option<i64> = sqlx::query_scalar(
            "DELETE FROM access_requests WHERE project = ? AND user_id = ? RETURNING user_id",
        )
        .bind(project)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(failed("delete access request"))?;
        if deleted.is_none() {
            return Ok(None);
        }
        let username = username_of(&mut transaction, user_id).await?;
        transaction.commit().await.map_err(failed("commit delete request"))?;
        Ok(Some(username))
    }
}

async fn ensure_registered(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &str,
) -> Result<(), StorageError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM project_registrations WHERE name = ?")
            .bind(project)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(failed("find project registration"))?;
    found
        .map(|_| ())
        .ok_or_else(|| StorageError::ProjectNotRegistered(project.to_string()))
}

async fn username_of(
    transaction: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> Result<String, StorageError> {
    let username: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(failed("find user"))?;
    username.ok_or(StorageError::IdentityNotFound(user_id))
}

async fn member_role(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &str,
    user_id: i64,
) -> Result<Option<Role>, StorageError> {
    let role: Option<String> =
        sqlx::query_scalar("SELECT role FROM project_members WHERE project = ? AND user_id = ?")
            .bind(project)
            .bind(user_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(failed("read member role"))?;
    role.map(|role| decode_role(&role)).transpose()
}

async fn owner_count(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &str,
) -> Result<i64, StorageError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM project_members WHERE project = ? AND role = 'owner'")
        .bind(project)
        .fetch_one(&mut **transaction)
        .await
        .map_err(failed("count owners"))
}

fn decode_member(row: &SqliteRow) -> Result<MemberRecord, StorageError> {
    let role: String = row.try_get("role").map_err(failed("decode member"))?;
    Ok(MemberRecord {
        user_id: row.try_get("user_id").map_err(failed("decode member"))?,
        username: row.try_get("username").map_err(failed("decode member"))?,
        role: decode_role(&role)?,
        granted_by: row.try_get("granted_by").map_err(failed("decode member"))?,
        granted_at: row.try_get("granted_at").map_err(failed("decode member"))?,
    })
}

fn decode_role(value: &str) -> Result<Role, StorageError> {
    Role::parse(value).ok_or_else(|| StorageError::InvalidData {
        operation: "decode role",
        message: format!("unknown role {value:?}"),
    })
}

fn failed(operation: &'static str) -> impl FnOnce(sqlx::Error) -> StorageError {
    move |source| StorageError::Operation { operation, source }
}
```

- [ ] **Step 5: Add registration with owner**

In `tools/filer-task-web/src/storage/projects.rs`, add this method inside `impl Storage`, after `insert_project_registration`:

```rust
    /// Registers a project and makes `owner_user_id` its owner in one
    /// transaction, so a crash cannot leave a registered project without an
    /// owner.
    pub async fn insert_project_registration_with_owner(
        &self,
        registration: &ProjectRegistration,
        owner_user_id: i64,
    ) -> Result<(), StorageError> {
        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "begin project registration",
                    source,
                })?;
        sqlx::query("INSERT INTO project_registrations (name, root) VALUES (?, ?)")
            .bind(&registration.name)
            .bind(encode_path(&registration.root))
            .execute(&mut *transaction)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "insert project registration",
                source,
            })?;
        sqlx::query(
            "INSERT INTO project_members (project, user_id, role, granted_by) \
             VALUES (?, ?, 'owner', ?)",
        )
        .bind(&registration.name)
        .bind(owner_user_id)
        .bind(owner_user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "insert project owner",
            source,
        })?;
        transaction
            .commit()
            .await
            .map_err(|source| StorageError::Operation {
                operation: "commit project registration",
                source,
            })?;
        Ok(())
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_storage_test`
Expected: PASS, 12 tests.

- [ ] **Step 7: Commit**

```bash
git add tools/filer-task-web/src/storage tools/filer-task-web/tests/membership_storage_test.rs
git commit -m "feat(task-web): store project members and access requests"
```

---

### Task 4: Guard every existing write route

**Files:**
- Create: `tools/filer-task-web/src/access.rs`
- Create: `tools/filer-task-web/tests/membership_api_test.rs`
- Modify: `tools/filer-task-web/src/lib.rs`
- Modify: `tools/filer-task-web/src/error.rs`
- Modify: `tools/filer-task-web/src/identity.rs`
- Modify: `tools/filer-task-web/src/app.rs`
- Modify: `tools/filer-task-web/src/routes/write.rs`
- Modify: `tools/filer-task-web/src/routes/task_writes.rs`
- Replace: `tools/filer-task-web/src/routes/transitions.rs`
- Modify: `tools/filer-task-web/src/routes/policy.rs`
- Modify: `tools/filer-task-web/src/routes/projects.rs`
- Modify: `tools/filer-task-web/tests/common/mod.rs`

**Interfaces:**
- Consumes: `Storage::project_role`, `Storage::grant_member`, `Storage::insert_project_registration_with_owner`, `Role`, `ResolvedSession::is_admin`, `StorageError::{LastOwner, AlreadyMember, ProjectNotRegistered, IdentityNotFound}`.
- Produces:
  - `filer_task_web::access::{ServerAdmin, ProjectEditor, ProjectOwner}`: `pub struct ServerAdmin(pub Actor)`, `pub struct ProjectEditor { pub actor: Actor, pub project: String }`, `pub struct ProjectOwner { pub actor: Actor, pub project: String }`, each `FromRequestParts<AppState>` with `Rejection = WebError`.
  - `Actor { user_id, username, session_id, is_admin: bool }`.
  - `WebError::{AdminRequired, ProjectRoleRequired(Role), LastOwner, AlreadyMember, AccessRequestNotFound, UserNotFound}`.
  - `AppState::project_registrations(&self) -> Vec<ProjectRegistration>`.
  - `routes::write::mutate(state: AppState, editor: ProjectEditor, action: &'static str, detail: Option<String>, operation: F)`.
  - Test scaffolding in `tests/membership_api_test.rs`: `TestApp { router, storage, project, repo, _database }`, `app()`, `TestApp::user(&self, username, Option<Role>) -> (String, i64)`, `TestApp::uri`, `TestApp::task_files`, `Guard`, `write_routes`, `request`, `send`, `snapshot`. Task 5 and 6 extend this file.

- [ ] **Step 1: Write the failing API tests**

Create `tools/filer-task-web/tests/membership_api_test.rs`:

```rust
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
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri} without a session: {body}");
        if guard == Guard::Session {
            continue;
        }
        let (status, body) = send(&app.router, request(method, &uri, None, Some(&stranger))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri} as a non-member: {body}");
        let expected = if guard == Guard::Admin {
            "admin_required"
        } else {
            "project_role_required"
        };
        assert_eq!(body["code"], expected, "{method} {uri}");
    }

    assert_eq!(app.task_files(), before, "no refused request changed a task file");
}

#[tokio::test]
async fn editors_write_tasks_but_not_owner_routes() {
    let app = app().await;
    app.user("Olivia", Some(Role::Owner)).await;
    let (editor, _) = app.user("Eve", Some(Role::Editor)).await;

    let (status, body) = send(
        &app.router,
        request("POST", &app.uri("/tasks/CORE-001/start"), None, Some(&editor)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["detail"]["task"]["status"], "In Progress");

    for (method, suffix) in [("PATCH", "/policy"), ("DELETE", "")] {
        let (status, body) =
            send(&app.router, request(method, &app.uri(suffix), None, Some(&editor))).await;
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

    let (status, summary) =
        send(&app.router, request("POST", "/api/projects", Some(body), Some(&admin))).await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    let name = summary["name"].as_str().expect("summary names the project");
    assert_eq!(
        app.storage.project_role(name, admin_id).await.expect("role reads"),
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
        .grant_member("My Project", session.identity.user_id, Role::Editor, None, false)
        .await
        .expect("editor grants");
    let cookie = format!("{IDENTITY_COOKIE}={}", session.session_token);

    let (status, body) = send(
        &router(state),
        request(
            "POST",
            "/api/projects/My%20Project/tasks/CORE-001/start",
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
        ("PATCH", app.uri("/tasks/CORE-001"), Guard::Editor),
        ("PUT", app.uri("/tasks/CORE-001/criteria/0"), Guard::Editor),
        ("POST", app.uri("/tasks/CORE-001/start"), Guard::Editor),
        ("POST", app.uri("/tasks/CORE-001/done"), Guard::Editor),
        ("POST", app.uri("/tasks/CORE-001/block"), Guard::Editor),
        ("POST", app.uri("/tasks/CORE-001/defer"), Guard::Editor),
        ("POST", app.uri("/tasks/CORE-001/obsolete"), Guard::Editor),
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_api_test`
Expected: FAIL to compile with `no method named `project_registrations` found for struct `AppState``.

- [ ] **Step 3: Add the web errors**

In `tools/filer-task-web/src/error.rs`, change the storage import to `use crate::{dto::ValidationIssue, storage::{Role, StorageError}};` and add variants to `WebError` after `SessionNotFound,`:
```rust
    AdminRequired,
    ProjectRoleRequired(Role),
    LastOwner,
    AlreadyMember,
    AccessRequestNotFound,
    UserNotFound,
```

Replace `impl From<StorageError> for WebError`:
```rust
impl From<StorageError> for WebError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::UsernameTaken => Self::UsernameTaken,
            StorageError::LastOwner => Self::LastOwner,
            StorageError::AlreadyMember => Self::AlreadyMember,
            StorageError::IdentityNotFound(_) => Self::UserNotFound,
            StorageError::ProjectNotRegistered(name) => Self::ProjectNotFound(name),
            other => Self::Storage(other),
        }
    }
}
```

`routes/identity.rs::rename_or_recover` matches `StorageError::IdentityNotFound` before converting, so the mapping above does not change rename recovery.

In `into_response`, add these arms after the `Self::SessionNotFound` arm:
```rust
            Self::AdminRequired => {
                return client_error(
                    StatusCode::FORBIDDEN,
                    "only server admins can register projects",
                    "admin_required",
                    None,
                );
            }
            Self::ProjectRoleRequired(role) => {
                return client_error(
                    StatusCode::FORBIDDEN,
                    format!("this change needs the {role} role on the project"),
                    "project_role_required",
                    None,
                );
            }
            Self::LastOwner => {
                return client_error(
                    StatusCode::CONFLICT,
                    "a project must keep at least one owner",
                    "last_owner",
                    None,
                );
            }
            Self::AlreadyMember => {
                return client_error(
                    StatusCode::CONFLICT,
                    "you are already a member of this project",
                    "already_member",
                    None,
                );
            }
            Self::AccessRequestNotFound => {
                return client_error(
                    StatusCode::NOT_FOUND,
                    "no pending access request for that user",
                    "access_request_not_found",
                    None,
                );
            }
            Self::UserNotFound => {
                return client_error(
                    StatusCode::NOT_FOUND,
                    "no user has that id",
                    "user_not_found",
                    None,
                );
            }
```

- [ ] **Step 4: Carry the admin flag on `Actor`**

In `tools/filer-task-web/src/identity.rs`, replace the struct and its `From` impl:
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub user_id: i64,
    pub username: String,
    pub session_id: i64,
    pub is_admin: bool,
}

impl From<ResolvedSession> for Actor {
    fn from(session: ResolvedSession) -> Self {
        Self {
            user_id: session.identity.user_id,
            username: session.identity.username,
            session_id: session.session_id,
            is_admin: session.is_admin,
        }
    }
}
```

- [ ] **Step 5: Write the access guards**

Create `tools/filer-task-web/src/access.rs`:

```rust
//! # Access Guards
//!
//! Extractors that turn a session into a permission check. A write handler
//! names the guard it needs in its signature. Each guard runs before body
//! extractors, so a forbidden request is rejected before its JSON is parsed or
//! the project's write lock is taken. Checks run in order: no session is 401,
//! an unregistered project is 404, and a missing role is 403.

use std::net::IpAddr;

use axum::{
    extract::{FromRequestParts, RawPathParams},
    http::request::Parts,
};

use crate::{app::AppState, error::WebError, identity::Actor, storage::Role};

/// A signed-in server admin. Admin controls project registration only.
pub struct ServerAdmin(pub Actor);

/// A signed-in user holding the editor or owner role on the `{project}` in the
/// request path.
pub struct ProjectEditor {
    pub actor: Actor,
    pub project: String,
}

/// A signed-in user holding the owner role on the `{project}` in the request
/// path.
pub struct ProjectOwner {
    pub actor: Actor,
    pub project: String,
}

impl FromRequestParts<AppState> for ServerAdmin {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let actor = Actor::from_request_parts(parts, state).await?;
        if !actor.is_admin {
            return Err(WebError::AdminRequired);
        }
        Ok(Self(actor))
    }
}

impl FromRequestParts<AppState> for ProjectEditor {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let (actor, project) = require_project_role(parts, state, Role::Editor).await?;
        Ok(Self { actor, project })
    }
}

impl FromRequestParts<AppState> for ProjectOwner {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let (actor, project) = require_project_role(parts, state, Role::Owner).await?;
        Ok(Self { actor, project })
    }
}

async fn require_project_role(
    parts: &mut Parts,
    state: &AppState,
    required: Role,
) -> Result<(Actor, String), WebError> {
    let actor = Actor::from_request_parts(parts, state).await?;
    let project = project_param(parts, state).await?;
    state.registry.resolve(&project)?;
    let role = state.storage().project_role(&project, actor.user_id).await?;
    if !role.is_some_and(|role| role.satisfies(required)) {
        return Err(WebError::ProjectRoleRequired(required));
    }
    Ok((actor, project))
}

// RawPathParams yields percent-decoded values, so the name matches the
// registry even when the URI encodes characters such as spaces.
async fn project_param(parts: &mut Parts, state: &AppState) -> Result<String, WebError> {
    let params = RawPathParams::from_request_parts(parts, state)
        .await
        .map_err(|rejection| WebError::BadRequest(rejection.body_text()))?;
    params
        .iter()
        .find(|(key, _)| *key == "project")
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| {
            WebError::Internal("a project guard is mounted on a route without {project}".to_string())
        })
}

/// The startup warning for a listen address other machines can reach, or
/// `None` for a loopback address.
pub fn exposure_warning(host: IpAddr) -> Option<String> {
    (!host.is_loopback()).then(|| {
        format!(
            "warning: listening on {host}: reads are open to anyone who can reach this address; writes need project membership"
        )
    })
}
```

In `tools/filer-task-web/src/lib.rs`, add `pub mod access;` as the first module line, above `pub mod app;`.

- [ ] **Step 6: Route registration through the owner insert and expose registrations**

In `tools/filer-task-web/src/app.rs`, change the storage import to `storage::{NewActivity, ProjectRegistration, Storage},`.

In `AppState::register_project`, replace:
```rust
        self.storage
            .insert_project_registration(&registration)
            .await?;
```
with:
```rust
        self.storage
            .insert_project_registration_with_owner(&registration, actor.user_id)
            .await?;
```

Add this method to `impl AppState`, after `storage()`:
```rust
    /// The registration of every project the registry holds, in registry order.
    pub fn project_registrations(&self) -> Vec<ProjectRegistration> {
        self.registry
            .names()
            .into_iter()
            .filter_map(|name| self.registry.resolve(&name).ok())
            .map(|project| project.registration())
            .collect()
    }
```

Add this comment directly above `pub fn router(state: AppState) -> Router {`:
```rust
// Every non-GET route below names its guard in the handler signature, and
// `write_routes` in tests/membership_api_test.rs lists each one with that
// guard. A non-GET route added here joins that table.
```

- [ ] **Step 7: Guard the task writes**

In `tools/filer-task-web/src/routes/write.rs`, replace the import block and the `mutate` signature through its first statement:
```rust
use crate::{
    access::ProjectEditor, app::AppState, error::WebError, routes::blocking, storage::NewActivity,
};

pub(crate) async fn mutate<F>(
    state: AppState,
    editor: ProjectEditor,
    action: &'static str,
    detail: Option<String>,
    operation: F,
) -> Result<ShowView, WebError>
where
    F: FnOnce(&TaskProject, &[Task]) -> Result<TaskIdentity, WebError> + Send + 'static,
{
    let ProjectEditor {
        actor,
        project: project_name,
    } = editor;
    let registered = state.registry.resolve(&project_name)?.clone();
```
Keep the rest of the function body unchanged; it already uses `actor` and `project_name`.

In `tools/filer-task-web/src/routes/task_writes.rs`, change `identity::Actor,` in the `crate` import to `access::ProjectEditor,` and change the three handler signatures and their `write::mutate` calls:

```rust
pub(crate) async fn create_task(
    State(state): State<AppState>,
    editor: ProjectEditor,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<ShowView>, WebError> {
```
with its call becoming:
```rust
    let view = write::mutate(
        state,
        editor,
        "task.create",
        None,
```

```rust
pub(crate) async fn edit_task(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<EditTaskRequest>,
) -> Result<Json<ShowView>, WebError> {
    let patch = body.into();
    let view = write::mutate(
        state,
        editor,
        "task.edit",
        None,
```

```rust
pub(crate) async fn set_criterion(
    State(state): State<AppState>,
    Path((_project, id, index)): Path<(String, String, usize)>,
    editor: ProjectEditor,
    headers: HeaderMap,
    Json(body): Json<SetCriterionRequest>,
) -> Result<Json<ShowView>, WebError> {
    let expected_hash = if_match(&headers)?;
    let detail = Some(format!("index {index} = {}", body.checked));
    let view = write::mutate(
        state,
        editor,
        "task.criterion",
        detail,
```

Replace `tools/filer-task-web/src/routes/transitions.rs` entirely:

```rust
//! # Transition Routes
//!
//! Each transition validates and resolves within its selected project. A
//! per-project lock keeps the mutation and refreshed response together without
//! serializing unrelated repositories.

use axum::{
    Json,
    extract::{Path, State},
};
use taskroot::{
    agent_context::ShowView,
    error::TaskError,
    identity::TaskIdentity,
    lifecycle::{block_task, defer_task, done_task, obsolete_task, start_task},
    project::TaskProject,
};

use crate::{
    access::ProjectEditor,
    app::AppState,
    dto::ReasonRequest,
    error::WebError,
    routes::{tasks::resolve_identity, write},
};

pub(crate) async fn start(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
) -> Result<Json<ShowView>, WebError> {
    transition(state, editor, id, "task.start", None, start_task).await
}

pub(crate) async fn done(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
) -> Result<Json<ShowView>, WebError> {
    transition(state, editor, id, "task.done", None, done_task).await
}

pub(crate) async fn block(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        editor,
        id,
        "task.block",
        detail,
        move |task_project, identity| block_task(task_project, identity, &reason),
    )
    .await
}

pub(crate) async fn defer(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        editor,
        id,
        "task.defer",
        detail,
        move |task_project, identity| defer_task(task_project, identity, &reason),
    )
    .await
}

pub(crate) async fn obsolete(
    State(state): State<AppState>,
    Path((_project, id)): Path<(String, String)>,
    editor: ProjectEditor,
    Json(body): Json<ReasonRequest>,
) -> Result<Json<ShowView>, WebError> {
    let reason = body.reason;
    let detail = Some(reason.clone());
    transition(
        state,
        editor,
        id,
        "task.obsolete",
        detail,
        move |task_project, identity| obsolete_task(task_project, identity, &reason),
    )
    .await
}

async fn transition<F>(
    state: AppState,
    editor: ProjectEditor,
    id: String,
    action: &'static str,
    detail: Option<String>,
    op: F,
) -> Result<Json<ShowView>, WebError>
where
    F: FnOnce(&TaskProject, &TaskIdentity) -> Result<std::path::PathBuf, TaskError>
        + Send
        + 'static,
{
    let view = write::mutate(state, editor, action, detail, move |project, tasks| {
        let identity = resolve_identity(project, tasks, &id)?;
        op(project, &identity)?;
        Ok(identity)
    })
    .await?;
    Ok(Json(view))
}
```

Before replacing, compare the original file's `start`, `done`, `block`, `defer`, and `obsolete` bodies against this version; the only intended differences are the guard, the dropped project argument, and the removed `#[allow(clippy::too_many_arguments)]`.

- [ ] **Step 8: Guard policy, registration, and deregistration**

In `tools/filer-task-web/src/routes/policy.rs`, change `identity::Actor,` in the `crate` import to `access::ProjectOwner,` and replace the `mutate_policy` signature and first line:
```rust
pub(crate) async fn mutate_policy(
    State(state): State<AppState>,
    owner: ProjectOwner,
    Json(request): Json<PolicyMutationRequest>,
) -> Result<Json<ProjectPolicyResponse>, WebError> {
    let ProjectOwner {
        actor,
        project: project_name,
    } = owner;
    let registered = state.registry.resolve(&project_name)?;
```
Keep the rest of the body unchanged.

In `tools/filer-task-web/src/routes/projects.rs`, change the axum import to:
```rust
use axum::{Json, extract::State, http::StatusCode};
```
change `identity::Actor,` in the `crate` import to `access::{ProjectOwner, ServerAdmin},`, change the `register_project` signature to:
```rust
pub(crate) async fn register_project(
    State(state): State<AppState>,
    ServerAdmin(actor): ServerAdmin,
    Json(request): Json<RegisterProjectRequest>,
) -> Result<Json<ProjectSummary>, WebError> {
```
and replace `deregister_project`:
```rust
pub(crate) async fn deregister_project(
    State(state): State<AppState>,
    owner: ProjectOwner,
) -> Result<StatusCode, WebError> {
    state.deregister_project(&owner.actor, &owner.project).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 9: Give the shared test identity ownership of every project**

Replace `tools/filer-task-web/tests/common/mod.rs`:

```rust
use axum::{
    Router,
    http::{HeaderValue, header::COOKIE},
};
use std::sync::atomic::{AtomicU64, Ordering};
use tower_http::set_header::SetRequestHeaderLayer;

use filer_task_web::{
    app::{AppState, router},
    identity::IDENTITY_COOKIE,
    storage::Role,
};

/// Builds a router whose requests carry a fresh test identity that owns every
/// project in `state`. Tests about other behavior then pass the membership
/// guards; membership tests build their own users instead.
pub async fn authenticated_router(state: AppState) -> Router {
    static NEXT_USER: AtomicU64 = AtomicU64::new(1);
    let username = format!("Test User {}", NEXT_USER.fetch_add(1, Ordering::Relaxed));
    let session = state
        .storage()
        .create_identity(&username, "Test browser")
        .await
        .expect("test identity creates");
    own_every_project(&state, session.identity.user_id).await;
    let cookie = HeaderValue::from_str(&format!("{IDENTITY_COOKIE}={}", session.session_token))
        .expect("test identity cookie is valid");
    router(state).layer(SetRequestHeaderLayer::if_not_present(COOKIE, cookie))
}

// Registries built from roots exist only in memory, but memberships reference
// persisted registrations, so missing registrations are persisted first.
async fn own_every_project(state: &AppState, user_id: i64) {
    let storage = state.storage();
    let persisted: Vec<String> = storage
        .project_registrations()
        .await
        .expect("registrations load")
        .into_iter()
        .map(|registration| registration.name)
        .collect();
    for registration in state.project_registrations() {
        if !persisted.contains(&registration.name) {
            storage
                .insert_project_registration(&registration)
                .await
                .expect("test registration persists");
        }
        storage
            .grant_member(&registration.name, user_id, Role::Owner, None, false)
            .await
            .expect("test owner grants");
    }
}
```

- [ ] **Step 10: Run the membership tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_api_test`
Expected: PASS, 4 tests.

- [ ] **Step 11: Run the whole crate's tests and lints**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu`
Expected: PASS. If an existing test fails, read its assertion before changing anything: a test that writes as a second identity created outside `authenticated_router` needs that identity granted a role with `storage.grant_member(...)`; a test asserting other behavior means the implementation is wrong.

Run: `cargo +stable-x86_64-pc-windows-gnu clippy --locked -p filer-task-web --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings`
Expected: no warnings.

- [ ] **Step 12: Commit**

```bash
git add tools/filer-task-web/src tools/filer-task-web/tests
git commit -m "feat(task-web): require project roles for writes and admin for registration"
```

---

### Task 5: Member and access-request routes

**Files:**
- Create: `tools/filer-task-web/src/routes/members.rs`
- Modify: `tools/filer-task-web/src/routes/mod.rs`
- Modify: `tools/filer-task-web/src/app.rs` (router)
- Test: `tools/filer-task-web/tests/membership_api_test.rs`

**Interfaces:**
- Consumes: `ProjectOwner`, `Actor`, `Storage::{list_members, grant_member, remove_member, list_access_requests, delete_access_request, request_access}`, `NewActivity`, `Role`, `GrantOutcome`, `RemovedMember`.
- Produces routes:
  - `GET /api/projects/{project}/members` → `{"members": [{"user_id", "username", "role", "granted_by", "granted_at"}]}`
  - `PUT /api/projects/{project}/members/{user_id}` body `{"role": "owner"|"editor"}` → members response
  - `DELETE /api/projects/{project}/members/{user_id}` → 204
  - `GET /api/projects/{project}/access-requests` → `{"requests": [{"user_id", "username", "requested_at"}]}`
  - `DELETE /api/projects/{project}/access-requests/{user_id}` → 204
  - `POST` and `DELETE /api/projects/{project}/access-request` → 204

- [ ] **Step 1: Extend the route table and write the failing tests**

In `tools/filer-task-web/tests/membership_api_test.rs`, rename the parameter `_other_user` to `other_user` in `write_routes` and append these rows to the returned `vec!`:
```rust
        ("PUT", app.uri(&format!("/members/{other_user}")), Guard::Owner),
        ("DELETE", app.uri(&format!("/members/{other_user}")), Guard::Owner),
        ("DELETE", app.uri(&format!("/access-requests/{other_user}")), Guard::Owner),
        ("POST", app.uri("/access-request"), Guard::Session),
        ("DELETE", app.uri("/access-request"), Guard::Session),
```

Append these tests:
```rust
#[tokio::test]
async fn owners_approve_change_and_revoke_members() {
    let app = app().await;
    let (owner, owner_id) = app.user("Olivia", Some(Role::Owner)).await;
    let (requester, requester_id) = app.user("Rae", None).await;

    let (status, _) = send(
        &app.router,
        request("POST", &app.uri("/access-request"), None, Some(&requester)),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) =
        send(&app.router, request("GET", &app.uri("/access-requests"), None, Some(&owner))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["requests"][0]["username"], "Rae");

    let member_uri = app.uri(&format!("/members/{requester_id}"));
    let (status, body) = send(
        &app.router,
        request("PUT", &member_uri, Some(json!({"role": "editor"})), Some(&owner)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["members"][1]["username"], "Rae");
    assert_eq!(body["members"][1]["role"], "editor");
    assert_eq!(body["members"][1]["granted_by"], "Olivia");
    let (_, body) =
        send(&app.router, request("GET", &app.uri("/access-requests"), None, Some(&owner))).await;
    assert_eq!(body["requests"], json!([]));

    let (status, body) = send(
        &app.router,
        request("PUT", &member_uri, Some(json!({"role": "owner"})), Some(&owner)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = send(&app.router, request("DELETE", &member_uri, None, Some(&owner))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = send(
        &app.router,
        request("DELETE", &app.uri(&format!("/members/{owner_id}")), None, Some(&owner)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "last_owner");

    let actions = activity_actions(&app).await;
    for action in ["access.request", "member.grant", "member.role", "member.revoke"] {
        assert!(actions.contains(&action.to_string()), "missing {action} in {actions:?}");
    }
}

#[tokio::test]
async fn owners_deny_requests_and_unknown_targets_are_named() {
    let app = app().await;
    let (owner, _) = app.user("Olivia", Some(Role::Owner)).await;
    let (requester, requester_id) = app.user("Rae", None).await;
    send(
        &app.router,
        request("POST", &app.uri("/access-request"), None, Some(&requester)),
    )
    .await;

    let deny_uri = app.uri(&format!("/access-requests/{requester_id}"));
    let (status, _) = send(&app.router, request("DELETE", &deny_uri, None, Some(&owner))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = send(&app.router, request("DELETE", &deny_uri, None, Some(&owner))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "access_request_not_found");
    let (status, body) = send(
        &app.router,
        request("PUT", &app.uri("/members/9999"), Some(json!({"role": "editor"})), Some(&owner)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "user_not_found");
    assert!(activity_actions(&app).await.contains(&"access.deny".to_string()));
}

#[tokio::test]
async fn members_cannot_request_access_and_requests_can_be_withdrawn() {
    let app = app().await;
    let (owner, _) = app.user("Olivia", Some(Role::Owner)).await;
    let (requester, _) = app.user("Rae", None).await;

    let (status, body) = send(
        &app.router,
        request("POST", &app.uri("/access-request"), None, Some(&owner)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "already_member");

    send(
        &app.router,
        request("POST", &app.uri("/access-request"), None, Some(&requester)),
    )
    .await;
    let (status, _) = send(
        &app.router,
        request("DELETE", &app.uri("/access-request"), None, Some(&requester)),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) =
        send(&app.router, request("GET", &app.uri("/access-requests"), None, Some(&owner))).await;
    assert_eq!(body["requests"], json!([]));
}

#[tokio::test]
async fn a_revoked_member_is_refused_on_their_next_write() {
    let app = app().await;
    let (owner, _) = app.user("Olivia", Some(Role::Owner)).await;
    let (editor, editor_id) = app.user("Eve", Some(Role::Editor)).await;
    let start = app.uri("/tasks/CORE-001/start");
    let (status, body) = send(&app.router, request("POST", &start, None, Some(&editor))).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    send(
        &app.router,
        request("DELETE", &app.uri(&format!("/members/{editor_id}")), None, Some(&owner)),
    )
    .await;
    let (status, body) = send(
        &app.router,
        request("POST", &app.uri("/tasks/CORE-001/done"), None, Some(&editor)),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "project_role_required");
}

#[tokio::test]
async fn a_renamed_member_is_listed_under_their_current_name() {
    let app = app().await;
    let (owner, _) = app.user("Olivia", Some(Role::Owner)).await;
    let (_, editor_id) = app.user("Rae", Some(Role::Editor)).await;

    app.storage
        .rename_identity(editor_id, "Rachel")
        .await
        .expect("rename succeeds");
    let (_, body) =
        send(&app.router, request("GET", &app.uri("/members"), None, Some(&owner))).await;

    assert_eq!(body["members"][1]["username"], "Rachel");
}

#[tokio::test]
async fn registering_a_removed_project_again_starts_with_only_its_registrant() {
    let app = app().await;
    let (owner, owner_id) = app.user("Olivia", Some(Role::Owner)).await;
    app.user("Eve", Some(Role::Editor)).await;

    let (status, _) = send(&app.router, request("DELETE", &app.uri(""), None, Some(&owner))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = send(
        &app.router,
        request("POST", "/api/projects", Some(json!({"path": app.repo.path()})), Some(&owner)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let members = app.storage.list_members(&app.project).await.expect("members list");
    assert_eq!(members.len(), 1);
    assert_eq!((members[0].user_id, members[0].role), (owner_id, Role::Owner));
}

async fn activity_actions(app: &TestApp) -> Vec<String> {
    let (status, body) = send(
        &app.router,
        request("GET", &format!("/api/activity?project={}", app.project), None, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array()
        .expect("activity is an array")
        .iter()
        .map(|entry| entry["action"].as_str().expect("action is text").to_string())
        .collect()
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_api_test`
Expected: FAIL. `owners_approve_change_and_revoke_members` gets `404` for `POST .../access-request`; the route walk fails on the first member route.

- [ ] **Step 3: Write the member routes**

Create `tools/filer-task-web/src/routes/members.rs`:

```rust
//! # Membership Routes
//!
//! Lists members, lets owners grant, change, and revoke roles and review access
//! requests, and lets any signed-in user ask for access to a project they
//! cannot change. Each change made here records an activity row, so the
//! project history shows who granted what.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    access::ProjectOwner,
    app::AppState,
    error::WebError,
    identity::Actor,
    storage::{AccessRequestRecord, MemberRecord, NewActivity, Role},
};

#[derive(Serialize)]
pub(crate) struct MemberView {
    user_id: i64,
    username: String,
    role: Role,
    granted_by: Option<String>,
    granted_at: i64,
}

#[derive(Serialize)]
pub(crate) struct MembersResponse {
    members: Vec<MemberView>,
}

#[derive(Serialize)]
pub(crate) struct AccessRequestView {
    user_id: i64,
    username: String,
    requested_at: i64,
}

#[derive(Serialize)]
pub(crate) struct AccessRequestsResponse {
    requests: Vec<AccessRequestView>,
}

#[derive(Deserialize)]
pub(crate) struct RoleRequest {
    role: Role,
}

pub(crate) async fn list_members(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<MembersResponse>, WebError> {
    state.registry.resolve(&project)?;
    members_response(&state, &project).await
}

pub(crate) async fn put_member(
    State(state): State<AppState>,
    Path((_project, user_id)): Path<(String, i64)>,
    owner: ProjectOwner,
    Json(body): Json<RoleRequest>,
) -> Result<Json<MembersResponse>, WebError> {
    let outcome = state
        .storage()
        .grant_member(&owner.project, user_id, body.role, Some(owner.actor.user_id), true)
        .await?;
    let action = match outcome.previous {
        None => Some("member.grant"),
        Some(previous) if previous != body.role => Some("member.role"),
        Some(_) => None,
    };
    if let Some(action) = action {
        let detail = format!("{} as {}", outcome.username, body.role);
        record(&state, &owner, action, Some(&detail)).await;
    }
    members_response(&state, &owner.project).await
}

pub(crate) async fn delete_member(
    State(state): State<AppState>,
    Path((_project, user_id)): Path<(String, i64)>,
    owner: ProjectOwner,
) -> Result<StatusCode, WebError> {
    let removed = state
        .storage()
        .remove_member(&owner.project, user_id, true)
        .await?;
    if let Some(removed) = removed {
        record(&state, &owner, "member.revoke", Some(&removed.username)).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_access_requests(
    State(state): State<AppState>,
    owner: ProjectOwner,
) -> Result<Json<AccessRequestsResponse>, WebError> {
    let requests = state
        .storage()
        .list_access_requests(&owner.project)
        .await?
        .into_iter()
        .map(request_view)
        .collect();
    Ok(Json(AccessRequestsResponse { requests }))
}

pub(crate) async fn deny_access_request(
    State(state): State<AppState>,
    Path((_project, user_id)): Path<(String, i64)>,
    owner: ProjectOwner,
) -> Result<StatusCode, WebError> {
    let username = state
        .storage()
        .delete_access_request(&owner.project, user_id)
        .await?
        .ok_or(WebError::AccessRequestNotFound)?;
    record(&state, &owner, "access.deny", Some(&username)).await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn request_access(
    State(state): State<AppState>,
    Path(project): Path<String>,
    actor: Actor,
) -> Result<StatusCode, WebError> {
    state.registry.resolve(&project)?;
    if state.storage().request_access(&project, actor.user_id).await? {
        state
            .storage()
            .record_committed_activity(NewActivity {
                user_id: actor.user_id,
                username: &actor.username,
                project: &project,
                task_id: None,
                action: "access.request",
                detail: None,
            })
            .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn withdraw_access_request(
    State(state): State<AppState>,
    Path(project): Path<String>,
    actor: Actor,
) -> Result<StatusCode, WebError> {
    state.registry.resolve(&project)?;
    state
        .storage()
        .delete_access_request(&project, actor.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn members_response(
    state: &AppState,
    project: &str,
) -> Result<Json<MembersResponse>, WebError> {
    let members = state
        .storage()
        .list_members(project)
        .await?
        .into_iter()
        .map(member_view)
        .collect();
    Ok(Json(MembersResponse { members }))
}

async fn record(state: &AppState, owner: &ProjectOwner, action: &str, detail: Option<&str>) {
    state
        .storage()
        .record_committed_activity(NewActivity {
            user_id: owner.actor.user_id,
            username: &owner.actor.username,
            project: &owner.project,
            task_id: None,
            action,
            detail,
        })
        .await;
}

fn member_view(member: MemberRecord) -> MemberView {
    MemberView {
        user_id: member.user_id,
        username: member.username,
        role: member.role,
        granted_by: member.granted_by,
        granted_at: member.granted_at,
    }
}

fn request_view(request: AccessRequestRecord) -> AccessRequestView {
    AccessRequestView {
        user_id: request.user_id,
        username: request.username,
        requested_at: request.requested_at,
    }
}
```

In `tools/filer-task-web/src/routes/mod.rs`, add `pub mod members;` after `pub mod identity;`.

- [ ] **Step 4: Mount the routes**

In `tools/filer-task-web/src/app.rs`, add these routes in `router` after the `/api/projects/{project}/policy` route:

```rust
        .route(
            "/api/projects/{project}/members",
            get(routes::members::list_members),
        )
        .route(
            "/api/projects/{project}/members/{user_id}",
            put(routes::members::put_member).delete(routes::members::delete_member),
        )
        .route(
            "/api/projects/{project}/access-requests",
            get(routes::members::list_access_requests),
        )
        .route(
            "/api/projects/{project}/access-requests/{user_id}",
            delete(routes::members::deny_access_request),
        )
        .route(
            "/api/projects/{project}/access-request",
            post(routes::members::request_access).delete(routes::members::withdraw_access_request),
        )
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_api_test`
Expected: PASS, 10 tests.

Run: `cargo +stable-x86_64-pc-windows-gnu clippy --locked -p filer-task-web --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add tools/filer-task-web/src tools/filer-task-web/tests/membership_api_test.rs
git commit -m "feat(task-web): add member and access request routes"
```

---

### Task 6: Role fields for the browser

**Files:**
- Modify: `tools/filer-task-web/src/dto.rs` (`ProjectSummary`)
- Modify: `tools/filer-task-web/src/registry.rs:129,143,153` (three `ProjectSummary` literals)
- Modify: `tools/filer-task-web/src/routes/tasks.rs` (`list_projects`)
- Modify: `tools/filer-task-web/src/routes/identity.rs` (`IdentityResponse`, three handlers)
- Modify: `tools/filer-task-web/tests/identity_api_test.rs:39,50`
- Modify: `tools/filer-task-web/tests/pairing_api_test.rs:76,91,100`
- Test: `tools/filer-task-web/tests/membership_api_test.rs`

**Interfaces:**
- Consumes: `Storage::user_project_access`, `Storage::is_admin`, `ProjectAccess`, `resolve_optional`.
- Produces: `ProjectSummary.my_role: Option<ProjectAccess>` serialized as `"owner"`, `"editor"`, `"pending"`, or `null`; `IdentityResponse.is_admin: bool` on `GET` and `PUT /api/identity` and `POST /api/identity/pair`.

- [ ] **Step 1: Write the failing tests**

Append to `tools/filer-task-web/tests/membership_api_test.rs`:
```rust
#[tokio::test]
async fn the_project_list_reports_each_users_role() {
    let app = app().await;
    let (owner, _) = app.user("Olivia", Some(Role::Owner)).await;
    let (editor, _) = app.user("Eve", Some(Role::Editor)).await;
    let (pending, pending_id) = app.user("Pat", None).await;
    app.storage
        .request_access(&app.project, pending_id)
        .await
        .expect("request records");
    let (stranger, _) = app.user("Sam", None).await;

    for (cookie, expected) in [
        (Some(owner.as_str()), json!("owner")),
        (Some(editor.as_str()), json!("editor")),
        (Some(pending.as_str()), json!("pending")),
        (Some(stranger.as_str()), Value::Null),
        (None, Value::Null),
    ] {
        let (status, body) = send(&app.router, request("GET", "/api/projects", None, cookie)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body[0]["my_role"], expected, "cookie {cookie:?}");
    }
}

#[tokio::test]
async fn identity_responses_report_the_admin_flag() {
    let app = app().await;
    let (owner, _) = app.user("Olivia", Some(Role::Owner)).await;
    let (editor, _) = app.user("Eve", Some(Role::Editor)).await;

    let (_, body) = send(&app.router, request("GET", "/api/identity", None, Some(&owner))).await;
    assert_eq!(body, json!({"username": "Olivia", "is_admin": true}));
    let (_, body) = send(&app.router, request("GET", "/api/identity", None, Some(&editor))).await;
    assert_eq!(body, json!({"username": "Eve", "is_admin": false}));
}
```

Update the existing exact-match assertions so they expect the flag. In `tools/filer-task-web/tests/identity_api_test.rs` at lines 39 and 50, and `tools/filer-task-web/tests/pairing_api_test.rs` at lines 76, 91, and 100, change `json!({"username": "Alice"})` to `json!({"username": "Alice", "is_admin": true})`. Alice is the first identity in each of those tests, so she is admin.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_api_test --test identity_api_test --test pairing_api_test`
Expected: FAIL. `the_project_list_reports_each_users_role` fails with `left: Null, right: String("owner")`.

- [ ] **Step 3: Add `my_role` to project summaries**

In `tools/filer-task-web/src/dto.rs`, add the field at the end of `ProjectSummary`, and import `crate::storage::ProjectAccess` at the top of the file:
```rust
    // Always serialized, so the browser can tell "no role" from an older server.
    pub my_role: Option<ProjectAccess>,
```

In `tools/filer-task-web/src/registry.rs`, add `my_role: None,` to each of the three `ProjectSummary { ... }` literals (inside `summary()` twice and `unavailable_summary()` once).

In `tools/filer-task-web/src/routes/tasks.rs`, change the axum import to include `http::HeaderMap`, add `use std::collections::HashMap;`, change the `crate` import to:
```rust
use crate::{
    app::AppState,
    dto::ProjectSummary,
    error::WebError,
    identity::resolve_optional,
    routes::blocking,
    storage::ProjectAccess,
};
```
and replace `list_projects`:
```rust
pub(crate) async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProjectSummary>>, WebError> {
    let registry = state.registry.clone();
    let mut projects = blocking(move || registry.summaries()).await?;
    let access = project_access(&state, &headers).await;
    for project in &mut projects {
        project.my_role = access.get(&project.name).copied();
    }
    Ok(Json(projects))
}

// Reads stay open, so a failed session or role lookup leaves every role
// unknown instead of failing the project list.
async fn project_access(state: &AppState, headers: &HeaderMap) -> HashMap<String, ProjectAccess> {
    let actor = match resolve_optional(headers, state).await {
        Ok(Some(actor)) => actor,
        Ok(None) => return HashMap::new(),
        Err(error) => {
            tracing::warn!(?error, "could not resolve the session for project roles");
            return HashMap::new();
        }
    };
    state
        .storage()
        .user_project_access(actor.user_id)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "could not load project roles");
            HashMap::new()
        })
}
```

- [ ] **Step 4: Add `is_admin` to identity responses**

In `tools/filer-task-web/src/routes/identity.rs`, add the field:
```rust
#[derive(Serialize)]
pub(crate) struct IdentityResponse {
    username: String,
    is_admin: bool,
}
```

Replace `get_identity`:
```rust
pub(crate) async fn get_identity(actor: Actor) -> Json<IdentityResponse> {
    Json(IdentityResponse {
        username: actor.username,
        is_admin: actor.is_admin,
    })
}
```

In `put_identity`, replace the final `Ok((...))` expression:
```rust
    let is_admin = state.storage().is_admin(identity.user_id).await?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(SET_COOKIE, identity_cookie(&token)?);
    Ok((
        response_headers,
        Json(IdentityResponse {
            username: identity.username,
            is_admin,
        }),
    ))
```
(the `let mut response_headers` and `insert` lines replace the existing two lines with the same content).

In `pair_identity`, replace the tail after the `match outcome` block:
```rust
    let is_admin = state.storage().is_admin(session.identity.user_id).await?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(SET_COOKIE, identity_cookie(&session.session_token)?);
    Ok((
        response_headers,
        Json(IdentityResponse {
            username: session.identity.username,
            is_admin,
        }),
    ))
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu`
Expected: PASS, including `project_registry_persistence_test`, whose closed-storage test still expects `GET /api/projects` to return `[]`.

- [ ] **Step 6: Commit**

```bash
git add tools/filer-task-web/src tools/filer-task-web/tests
git commit -m "feat(task-web): report project roles and the admin flag to the browser"
```

---

### Task 7: CLI commands and the exposure warning

**Files:**
- Modify: `tools/filer-task-web/src/storage/identities.rs` (add `user_by_name`, `set_admin`)
- Modify: `tools/filer-task-web/src/main.rs`
- Create: `tools/filer-task-web/tests/membership_cli_test.rs`

**Interfaces:**
- Consumes: `Storage::{grant_member, remove_member, list_members, list_access_requests, admin_count, project_registrations}`, `Role::parse`, `access::exposure_warning`.
- Produces: `Storage::user_by_name(&self, username: &str) -> Result<Option<StoredIdentity>, StorageError>`; `Storage::set_admin(&self, username: &str, is_admin: bool) -> Result<Option<StoredIdentity>, StorageError>`; subcommands `admin-grant`, `admin-revoke`, `member-grant`, `member-revoke`, `member-list`; the startup warning on stderr.

- [ ] **Step 1: Write the failing tests**

Create `tools/filer-task-web/tests/membership_cli_test.rs`:

```rust
//! Exercises the membership subcommands of the `filer-task-web` binary end to
//! end against a temp database, and the startup warning for exposed hosts.

mod cli;

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use cli::run;
use filer_task_web::{
    access::exposure_warning,
    storage::{ProjectRegistration, Role, Storage},
};
use tempfile::TempDir;

fn run_ok(args: &[&str]) -> Output {
    let output = run(args);
    assert!(
        output.status.success(),
        "expected success, got {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Alice (admin, owner of alpha) and Bob (no role) in a fresh database.
async fn seeded() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3");
    let storage = Storage::open(&db).await.expect("storage opens");
    let alice = storage
        .create_identity("Alice", "Test browser")
        .await
        .expect("alice creates")
        .identity
        .user_id;
    storage
        .create_identity("Bob", "Test browser")
        .await
        .expect("bob creates");
    storage
        .insert_project_registration_with_owner(
            &ProjectRegistration::new("alpha", temp.path().join("alpha")),
            alice,
        )
        .await
        .expect("alpha registers");
    storage.close().await;
    (temp, db)
}

async fn open(db: &Path) -> Storage {
    Storage::open(db).await.expect("storage opens")
}

#[tokio::test]
async fn admin_grant_and_revoke_flip_the_flag() {
    let (_temp, db) = seeded().await;
    let db_arg = db.display().to_string();

    let granted = run_ok(&["admin-grant", "bob", "--database", &db_arg]);
    assert_eq!(stdout(&granted).trim(), "granted admin to Bob");
    let storage = open(&db).await;
    let bob = storage.user_by_name("Bob").await.expect("lookup").expect("bob exists");
    assert!(storage.is_admin(bob.user_id).await.expect("flag reads"));
    storage.close().await;

    run_ok(&["admin-revoke", "Bob", "--database", &db_arg]);
    let last = run_ok(&["admin-revoke", "Alice", "--database", &db_arg]);
    assert!(stderr(&last).contains("no admins remain"), "{}", stderr(&last));

    let missing = run(&["admin-grant", "Nobody", "--database", &db_arg]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("no user named \"Nobody\""), "{}", stderr(&missing));
}

#[tokio::test]
async fn member_commands_grant_list_and_revoke() {
    let (_temp, db) = seeded().await;
    let db_arg = db.display().to_string();
    let storage = open(&db).await;
    let bob = storage.user_by_name("Bob").await.expect("lookup").expect("bob exists");
    storage.request_access("alpha", bob.user_id).await.expect("request records");
    storage.close().await;

    let granted = run_ok(&["member-grant", "alpha", "Bob", "--role", "editor", "--database", &db_arg]);
    assert_eq!(stdout(&granted).trim(), "Bob is editor of alpha");
    let listed = run_ok(&["member-list", "alpha", "--database", &db_arg]);
    assert_eq!(stdout(&listed), "owner\tAlice\neditor\tBob\n");
    let storage = open(&db).await;
    assert!(storage.list_access_requests("alpha").await.expect("requests list").is_empty());
    storage.close().await;

    let revoked = run_ok(&["member-revoke", "alpha", "Alice", "--database", &db_arg]);
    assert!(stderr(&revoked).contains("alpha has no owner"), "{}", stderr(&revoked));
    let listed = run_ok(&["member-list", "alpha", "--database", &db_arg]);
    assert_eq!(stdout(&listed), "editor\tBob\n");

    let storage = open(&db).await;
    assert_eq!(
        storage.project_role("alpha", bob.user_id).await.expect("role reads"),
        Some(Role::Editor)
    );
    storage.close().await;

    // as_str, not &db_arg: these arrays have no expected element type, so a
    // &String would not coerce to &str.
    let db_arg = db_arg.as_str();
    for (args, message) in [
        (
            vec!["member-grant", "ghost", "Bob", "--role", "editor", "--database", db_arg],
            "project ghost is not registered",
        ),
        (
            vec!["member-grant", "alpha", "Bob", "--role", "admin", "--database", db_arg],
            "invalid --role value \"admin\"",
        ),
        (
            vec!["member-grant", "alpha", "Bob", "--database", db_arg],
            "--role is required",
        ),
        (vec!["member-list", "--database", db_arg], "missing project"),
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "expected failure for {args:?}");
        assert!(stderr(&output).contains(message), "{args:?}: {}", stderr(&output));
    }
}

#[test]
fn the_exposure_warning_names_only_non_loopback_hosts() {
    for host in ["0.0.0.0", "192.168.1.20", "::"] {
        let warning = exposure_warning(host.parse().expect("address parses"))
            .unwrap_or_else(|| panic!("{host} should warn"));
        assert!(warning.contains(host), "{warning}");
        assert!(warning.contains("writes need project membership"), "{warning}");
    }
    for host in ["127.0.0.1", "::1"] {
        assert_eq!(exposure_warning(host.parse().expect("address parses")), None, "{host}");
    }
}

#[test]
fn serving_on_the_loopback_default_prints_no_warning() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3").display().to_string();
    let mut child = Command::new(cli::binary())
        .args(["--port", "0", "--database", &db])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server process spawns");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout is piped"))
        .read_line(&mut line)
        .expect("stdout is readable");
    assert!(line.starts_with("filer-task-web listening on"), "{line}");
    child.kill().expect("server stops");
    child.wait().expect("server exits");
    let mut errors = String::new();
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut errors)
        .expect("stderr is readable");
    assert!(!errors.contains("warning"), "{errors}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_cli_test`
Expected: FAIL to compile with `no method named `user_by_name` found for struct `Storage``.

- [ ] **Step 3: Add the identity lookups**

In `tools/filer-task-web/src/storage/identities.rs`, add inside `impl Storage`, after `admin_count`:
```rust
    pub async fn user_by_name(&self, username: &str) -> Result<Option<StoredIdentity>, StorageError> {
        let row = sqlx::query("SELECT id, display_name FROM users WHERE name_key = ?")
            .bind(username.to_lowercase())
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "find user by name",
                source,
            })?;
        row.map(|row| decode_identity(&row, "decode user by name"))
            .transpose()
    }

    /// Sets the admin flag for `username`. Returns `None` when no user has
    /// that name.
    pub async fn set_admin(
        &self,
        username: &str,
        is_admin: bool,
    ) -> Result<Option<StoredIdentity>, StorageError> {
        let row = sqlx::query(
            "UPDATE users SET is_admin = ?, updated_at = unixepoch() \
             WHERE name_key = ? RETURNING id, display_name",
        )
        .bind(is_admin)
        .bind(username.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "set admin flag",
            source,
        })?;
        row.map(|row| decode_identity(&row, "decode admin user"))
            .transpose()
    }
```

- [ ] **Step 4: Add the subcommands and the warning**

In `tools/filer-task-web/src/main.rs`:

Append this paragraph to the end of the module doc comment, after its last line `//! database.`, separated from it by a `//!` line:
```rust
//!
//! Membership subcommands are the operator override for project roles:
//! `admin-grant` and `admin-revoke` set the server admin flag, `member-grant`
//! and `member-revoke` change a user's role on a project, and `member-list`
//! prints a project's members and pending requests. They may remove the last
//! owner or admin, and warn when they do.
```

Change the `filer_task_web` import to:
```rust
use filer_task_web::{
    access::exposure_warning,
    app::{AppState, router},
    storage::{Role, Storage},
};
```

Add arms to the `match command` in `main`, after `SessionClear`:
```rust
        Command::AdminGrant { username, database } => admin_set(&username, &database, true).await,
        Command::AdminRevoke { username, database } => {
            admin_set(&username, &database, false).await
        }
        Command::MemberGrant {
            project,
            username,
            role,
            database,
        } => member_grant(&project, &username, role, &database).await,
        Command::MemberRevoke {
            project,
            username,
            database,
        } => member_revoke(&project, &username, &database).await,
        Command::MemberList { project, database } => member_list(&project, &database).await,
```

In `serve`, insert the warning between the bind and the announcement, so it is written before the announcement line a caller waits for:
```rust
    let listener = tokio::net::TcpListener::bind(addr).await?;
    if let Some(warning) = exposure_warning(options.host) {
        eprintln!("{warning}");
    }
```

Add these functions after `session_clear`:
```rust
async fn admin_set(
    username: &str,
    database: &PathBuf,
    is_admin: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    let identity = storage
        .set_admin(username, is_admin)
        .await?
        .ok_or_else(|| format!("no user named {username:?}"))?;
    if is_admin {
        println!("granted admin to {}", identity.username);
    } else {
        println!("revoked admin from {}", identity.username);
        if storage.admin_count().await? == 0 {
            eprintln!(
                "warning: no admins remain, so nobody can register projects until admin-grant runs again"
            );
        }
    }
    Ok(())
}

async fn member_grant(
    project: &str,
    username: &str,
    role: Role,
    database: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    let user = storage
        .user_by_name(username)
        .await?
        .ok_or_else(|| format!("no user named {username:?}"))?;
    storage
        .grant_member(project, user.user_id, role, None, false)
        .await?;
    println!("{} is {role} of {project}", user.username);
    warn_if_ownerless(&storage, project).await
}

async fn member_revoke(
    project: &str,
    username: &str,
    database: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    ensure_registered(&storage, project).await?;
    let user = storage
        .user_by_name(username)
        .await?
        .ok_or_else(|| format!("no user named {username:?}"))?;
    match storage.remove_member(project, user.user_id, false).await? {
        Some(removed) => println!("removed {} ({}) from {project}", removed.username, removed.role),
        None => println!("{} was not a member of {project}", user.username),
    }
    warn_if_ownerless(&storage, project).await
}

async fn member_list(project: &str, database: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    ensure_registered(&storage, project).await?;
    for member in storage.list_members(project).await? {
        println!("{}\t{}", member.role, member.username);
    }
    for request in storage.list_access_requests(project).await? {
        println!("pending\t{}", request.username);
    }
    Ok(())
}

async fn ensure_registered(storage: &Storage, project: &str) -> Result<(), Box<dyn std::error::Error>> {
    let registered = storage
        .project_registrations()
        .await?
        .iter()
        .any(|registration| registration.name == project);
    if registered {
        Ok(())
    } else {
        Err(format!("project {project} is not registered").into())
    }
}

async fn warn_if_ownerless(storage: &Storage, project: &str) -> Result<(), Box<dyn std::error::Error>> {
    let has_owner = storage
        .list_members(project)
        .await?
        .iter()
        .any(|member| member.role == Role::Owner);
    if !has_owner {
        eprintln!(
            "warning: {project} has no owner; run member-grant {project} <username> --role owner to assign one"
        );
    }
    Ok(())
}
```

Extend the `Command` enum:
```rust
enum Command {
    Serve(ServeOptions),
    SessionMint { username: String, database: PathBuf },
    SessionClear { username: String, database: PathBuf },
    AdminGrant { username: String, database: PathBuf },
    AdminRevoke { username: String, database: PathBuf },
    MemberGrant { project: String, username: String, role: Role, database: PathBuf },
    MemberRevoke { project: String, username: String, database: PathBuf },
    MemberList { project: String, database: PathBuf },
}
```

Replace `USAGE`:
```rust
const USAGE: &str = "\
usage:
  filer-task-web [--host <address>] [--port <port>] [--database <path>]
  filer-task-web session-mint <username> [--database <path>]
  filer-task-web session-clear <username> [--database <path>]
  filer-task-web admin-grant <username> [--database <path>]
  filer-task-web admin-revoke <username> [--database <path>]
  filer-task-web member-grant <project> <username> --role owner|editor [--database <path>]
  filer-task-web member-revoke <project> <username> [--database <path>]
  filer-task-web member-list <project> [--database <path>]";
```

Add arms to the `match` in `parse_args`, after `session-clear`:
```rust
        Some("admin-grant") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::AdminGrant { username, database }),
        Some("admin-revoke") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::AdminRevoke { username, database }),
        Some("member-grant") => {
            parse_member_subcommand(&args[1..], &["project", "username"], true).and_then(|parsed| {
                let role = parsed.role.ok_or_else(|| "--role is required".to_string())?;
                let [project, username] = <[String; 2]>::try_from(parsed.positionals)
                    .map_err(|_| "expected a project and a username".to_string())?;
                Ok(Command::MemberGrant {
                    project,
                    username,
                    role,
                    database: parsed.database,
                })
            })
        }
        Some("member-revoke") => {
            parse_member_subcommand(&args[1..], &["project", "username"], false).and_then(|parsed| {
                let [project, username] = <[String; 2]>::try_from(parsed.positionals)
                    .map_err(|_| "expected a project and a username".to_string())?;
                Ok(Command::MemberRevoke {
                    project,
                    username,
                    database: parsed.database,
                })
            })
        }
        Some("member-list") => {
            parse_member_subcommand(&args[1..], &["project"], false).and_then(|parsed| {
                let [project] = <[String; 1]>::try_from(parsed.positionals)
                    .map_err(|_| "expected a project".to_string())?;
                Ok(Command::MemberList {
                    project,
                    database: parsed.database,
                })
            })
        }
```

Add the parser after `parse_session_subcommand`:
```rust
struct MemberArgs {
    positionals: Vec<String>,
    role: Option<Role>,
    database: PathBuf,
}

fn parse_member_subcommand(
    args: &[String],
    names: &[&str],
    accepts_role: bool,
) -> Result<MemberArgs, String> {
    let mut database = PathBuf::from(DEFAULT_DATABASE);
    let mut role = None;
    let mut positionals = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--database" => {
                database = PathBuf::from(flag_value(args, index, "--database")?);
                index += 2;
            }
            "--role" if accepts_role => {
                let value = flag_value(args, index, "--role")?;
                role = Some(Role::parse(value).ok_or_else(|| {
                    format!("invalid --role value {value:?}, expected owner or editor")
                })?);
                index += 2;
            }
            flag if flag.starts_with('-') => return Err(format!("unexpected flag {flag:?}")),
            positional => {
                if positionals.len() == names.len() {
                    return Err(format!("unexpected argument {positional:?}"));
                }
                positionals.push(positional.to_string());
                index += 1;
            }
        }
    }
    if let Some(missing) = names.get(positionals.len()) {
        return Err(format!("missing {missing}"));
    }
    Ok(MemberArgs {
        positionals,
        role,
        database,
    })
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test membership_cli_test --test recovery_cli_test --test serve_cli_test`
Expected: PASS.

Run: `cargo +stable-x86_64-pc-windows-gnu clippy --locked -p filer-task-web --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add tools/filer-task-web/src tools/filer-task-web/tests/membership_cli_test.rs
git commit -m "feat(task-web): add membership CLI commands and an exposure warning"
```

---

### Task 8: Browser permission logic and API client

**Files:**
- Create: `tools/filer-task-web/static/js/lib/access.js`
- Create: `tools/filer-task-web/static/js/lib/activity.js`
- Modify: `tools/filer-task-web/static/js/lib/palette.js`
- Modify: `tools/filer-task-web/static/js/api/client.js`
- Modify: `tools/filer-task-web/static/js/store/project.js`
- Create: `tools/filer-task-web/tests/js/access.test.js`
- Create: `tools/filer-task-web/tests/js/activity.test.js`
- Create: `tools/filer-task-web/tests/js/client.test.js`
- Modify: `tools/filer-task-web/tests/js/rejection.test.js`
- Modify: `tools/filer-task-web/tests/js/palette.test.js`

**Interfaces:**
- Consumes: the JSON shapes from Tasks 5 and 6.
- Produces:
  - `access.js`: `ACTIONS` (`WRITE_TASKS`, `MANAGE_MEMBERS`, `EDIT_POLICY`, `REGISTER_PROJECTS`, `REQUEST_ACCESS`), `can(role, isAdmin, action) -> boolean`, `accessBanner(role) -> "request" | "pending" | null`, `isLastOwner(member, members) -> boolean`.
  - `activity.js`: `activityLabel(action) -> string`.
  - `palette.js`: `paletteRows(projects, query, { canCreate = true } = {})`.
  - `client.js`: `onAccessRejected(handler)`; `projectScoped(name)` gains `listMembers()`, `setMemberRole(userId, role)`, `removeMember(userId)`, `listAccessRequests()`, `denyAccessRequest(userId)`, `requestAccess()`, `withdrawAccessRequest()`.
  - `store/project.js` reloads projects when an access rejection arrives.

- [ ] **Step 1: Write the failing JS tests**

Create `tools/filer-task-web/tests/js/access.test.js`:
```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { ACTIONS, accessBanner, can, isLastOwner } from "../../static/js/lib/access.js";

// One row per line of the spec's permissions table.
const TABLE = [
  [ACTIONS.REQUEST_ACCESS, { none: true, pending: false, editor: false, owner: false }],
  [ACTIONS.WRITE_TASKS, { none: false, pending: false, editor: true, owner: true }],
  [ACTIONS.MANAGE_MEMBERS, { none: false, pending: false, editor: false, owner: true }],
  [ACTIONS.EDIT_POLICY, { none: false, pending: false, editor: false, owner: true }],
];

test("project permissions follow the role, not the admin flag", () => {
  for (const [action, expected] of TABLE) {
    for (const isAdmin of [false, true]) {
      assert.equal(can(null, isAdmin, action), expected.none, `${action} for a non-member`);
      assert.equal(can("pending", isAdmin, action), expected.pending, `${action} while pending`);
      assert.equal(can("editor", isAdmin, action), expected.editor, `${action} for an editor`);
      assert.equal(can("owner", isAdmin, action), expected.owner, `${action} for an owner`);
    }
  }
});

test("only admins register projects, whatever their project role", () => {
  for (const role of [null, "pending", "editor", "owner"]) {
    assert.equal(can(role, true, ACTIONS.REGISTER_PROJECTS), true);
    assert.equal(can(role, false, ACTIONS.REGISTER_PROJECTS), false);
  }
});

test("the banner asks non-members to request access and shows a pending request", () => {
  assert.equal(accessBanner(null), "request");
  assert.equal(accessBanner(undefined), "request");
  assert.equal(accessBanner("pending"), "pending");
  assert.equal(accessBanner("editor"), null);
  assert.equal(accessBanner("owner"), null);
});

test("only the sole owner counts as the last owner", () => {
  const alone = [{ role: "owner" }, { role: "editor" }];
  const shared = [{ role: "owner" }, { role: "owner" }];
  assert.equal(isLastOwner(alone[0], alone), true);
  assert.equal(isLastOwner(alone[1], alone), false);
  assert.equal(isLastOwner(shared[0], shared), false);
});
```

Create `tools/filer-task-web/tests/js/activity.test.js`:
```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { activityLabel } from "../../static/js/lib/activity.js";

test("membership actions read as phrases and task actions keep their names", () => {
  assert.equal(activityLabel("member.grant"), "granted access");
  assert.equal(activityLabel("member.role"), "changed role");
  assert.equal(activityLabel("member.revoke"), "removed member");
  assert.equal(activityLabel("access.request"), "requested access");
  assert.equal(activityLabel("access.deny"), "denied access");
  assert.equal(activityLabel("task.done"), "task.done");
});
```

Create `tools/filer-task-web/tests/js/client.test.js`:
```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError, onAccessRejected, projectScoped } from "../../static/js/api/client.js";

function respondWith(status, body) {
  globalThis.fetch = async () => ({
    ok: status >= 200 && status < 300,
    status,
    text: async () => (body === null ? "" : JSON.stringify(body)),
  });
}

test("a missing-role refusal notifies the access handler and still rejects", async () => {
  const seen = [];
  onAccessRejected((error) => seen.push(error.code));
  respondWith(403, {
    error: "this change needs the editor role on the project",
    code: "project_role_required",
  });

  await assert.rejects(
    projectScoped("demo").transition("CORE-001", "start"),
    (error) => error instanceof ApiError && error.status === 403,
  );
  assert.deepEqual(seen, ["project_role_required"]);
});

test("other refusals leave the access handler alone", async () => {
  const seen = [];
  onAccessRejected((error) => seen.push(error.code));
  respondWith(409, { error: "a project must keep at least one owner", code: "last_owner" });

  await assert.rejects(projectScoped("demo").removeMember(1));
  assert.deepEqual(seen, []);
});
```

Append to `tools/filer-task-web/tests/js/rejection.test.js`:
```js
test("membership refusals are form-level messages", () => {
  for (const [status, code] of [
    [403, "project_role_required"],
    [403, "admin_required"],
    [409, "last_owner"],
  ]) {
    const refused = new ApiError(status, { error: `refused: ${code}`, code });
    assert.deepEqual(fieldError(refused), {
      field: null,
      message: `refused: ${code}`,
      allowed: [],
    });
  }
});
```

Append to `tools/filer-task-web/tests/js/palette.test.js`:
```js
test("the create row is offered only to users who can register projects", () => {
  assert.deepEqual(paletteRows(PROJECTS, "brand-new"), [{ kind: "create", name: "brand-new" }]);
  assert.deepEqual(paletteRows(PROJECTS, "brand-new", { canCreate: false }), []);
  assert.deepEqual(
    paletteRows(PROJECTS, "filer", { canCreate: false }).map((row) => row.project.name),
    ["filer", "Filer-Docs"],
  );
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd tools/filer-task-web && node --test "tests/js/*.test.js"; cd ../..`
Expected: FAIL with `Cannot find module` for `static/js/lib/access.js`.

- [ ] **Step 3: Write the permission and label modules**

Create `tools/filer-task-web/static/js/lib/access.js`:
```js
// Permission rules the browser applies before it offers a control. The server
// enforces the same rules on every request; asking here only hides controls a
// request would refuse. Admin governs registration alone, so project actions
// depend on the role and never on the admin flag.

export const ACTIONS = Object.freeze({
  WRITE_TASKS: "write-tasks",
  MANAGE_MEMBERS: "manage-members",
  EDIT_POLICY: "edit-policy",
  REGISTER_PROJECTS: "register-projects",
  REQUEST_ACCESS: "request-access",
});

export function can(role, isAdmin, action) {
  switch (action) {
    case ACTIONS.WRITE_TASKS:
      return role === "owner" || role === "editor";
    case ACTIONS.MANAGE_MEMBERS:
    case ACTIONS.EDIT_POLICY:
      return role === "owner";
    case ACTIONS.REGISTER_PROJECTS:
      return isAdmin === true;
    case ACTIONS.REQUEST_ACCESS:
      return role === null || role === undefined;
    default:
      return false;
  }
}

// Which banner a project screen shows: "request" for a non-member, "pending"
// while a request waits, and null once the user can write.
export function accessBanner(role) {
  if (role === "owner" || role === "editor") {
    return null;
  }
  return role === "pending" ? "pending" : "request";
}

// A project keeps at least one owner, so the last owner's role and removal
// controls stay disabled.
export function isLastOwner(member, members) {
  return member.role === "owner" && members.filter((other) => other.role === "owner").length === 1;
}
```

Create `tools/filer-task-web/static/js/lib/activity.js`:
```js
// Readable labels for membership actions in the activity log. Task actions
// keep their machine names, which already read as verbs, such as task.done.
const LABELS = Object.freeze({
  "member.grant": "granted access",
  "member.role": "changed role",
  "member.revoke": "removed member",
  "access.request": "requested access",
  "access.deny": "denied access",
});

export function activityLabel(action) {
  return LABELS[action] ?? action;
}
```

- [ ] **Step 4: Gate the palette's create row**

In `tools/filer-task-web/static/js/lib/palette.js`, replace the comment and function `paletteRows`:
```js
// One flat row list so arrow keys and Enter treat the create action like any
// other row. A query that names nothing registered becomes the proposed name
// for a new project, which the create dialog then asks the user to confirm
// along with where it should live. Only server admins register projects, so
// the create row appears only when `canCreate` is set.
export function paletteRows(projects, query, { canCreate = true } = {}) {
  const matches = filterProjects(projects, query);
  if (matches.length > 0) {
    return matches.map((project) => ({ kind: "project", project }));
  }
  const name = (query ?? "").trim();
  return name === "" || !canCreate ? [] : [{ kind: "create", name }];
}
```

- [ ] **Step 5: Extend the API client**

In `tools/filer-task-web/static/js/api/client.js`, insert after the `ApiError` class:
```js
// Codes that mean this browser's view of its own permissions is stale. The
// project store registers a handler that reloads roles when one arrives.
const ACCESS_REJECTION_CODES = new Set(["project_role_required", "admin_required"]);
let accessRejectionHandler = null;

export function onAccessRejected(handler) {
  accessRejectionHandler = handler;
}
```

In `request`, replace:
```js
  if (!response.ok) {
    throw new ApiError(response.status, payload);
  }
```
with:
```js
  if (!response.ok) {
    const error = new ApiError(response.status, payload);
    if (accessRejectionHandler && ACCESS_REJECTION_CODES.has(error.code)) {
      accessRejectionHandler(error);
    }
    throw error;
  }
```

In `projectScoped`, add after `transition(...) { ... },`:
```js
    listMembers() {
      return getJson(`${base}/members`);
    },
    setMemberRole(userId, role) {
      return putJson(`${base}/members/${userId}`, { role });
    },
    removeMember(userId) {
      return request("DELETE", `${base}/members/${userId}`);
    },
    listAccessRequests() {
      return getJson(`${base}/access-requests`);
    },
    denyAccessRequest(userId) {
      return request("DELETE", `${base}/access-requests/${userId}`);
    },
    requestAccess() {
      return postJson(`${base}/access-request`);
    },
    withdrawAccessRequest() {
      return request("DELETE", `${base}/access-request`);
    },
```

- [ ] **Step 6: Reload roles after an access rejection**

In `tools/filer-task-web/static/js/store/project.js`, change the client import to `import { listProjects, onAccessRejected } from "../api/client.js";` and add after the `loadProjects` function:
```js
// A refusal for a missing role means the roles this store holds are stale;
// reloading them hides the controls the user has lost.
onAccessRejected(() => {
  loadProjects();
});
```

- [ ] **Step 7: Run the JS tests to verify they pass**

Run: `cd tools/filer-task-web && node --test "tests/js/*.test.js"; cd ../..`
Expected: PASS, all files.

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu --test frontend_js_test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add tools/filer-task-web/static/js tools/filer-task-web/tests/js
git commit -m "feat(task-web): add browser permission rules and member API calls"
```

---

### Task 9: Browser controls

**Files:**
- Create: `tools/filer-task-web/static/js/components/AccessBanner.js`
- Create: `tools/filer-task-web/static/js/components/MemberList.js`
- Create: `tools/filer-task-web/static/js/components/AccessRequests.js`
- Replace: `tools/filer-task-web/static/js/screens/Settings.js`
- Modify: `tools/filer-task-web/static/js/app.js`
- Modify: `tools/filer-task-web/static/js/components/Sidebar.js`
- Modify: `tools/filer-task-web/static/js/components/TaskDrawer.js`
- Modify: `tools/filer-task-web/static/js/components/DrawerCriteria.js`
- Modify: `tools/filer-task-web/static/js/components/CommandPalette.js`
- Modify: `tools/filer-task-web/static/js/screens/Activity.js`
- Modify: `tools/filer-task-web/static/style.css`

**Interfaces:**
- Consumes: everything Task 8 produces; `useIdentityStore().identity.is_admin`; `activeProject().my_role`.
- Produces: `AccessBanner({ project })`, `MemberList({ projectName, canManage, reloadKey })`, `AccessRequests({ projectName, onApproved })`; `TaskDrawer` takes `canWrite`; `DrawerCriteria` takes `readOnly`.

Components have no unit tests in this codebase; their rules come from the functions Task 8 tested, and Step 11 checks them in a browser.

- [ ] **Step 1: Write the access banner**

Create `tools/filer-task-web/static/js/components/AccessBanner.js`:
```js
import { html, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { accessBanner } from "../lib/access.js";
import { loadProjects } from "../store/project.js";

// Shown above a project's screens when the user cannot change it. Reloading
// the project list after a request or withdrawal refreshes my_role, which is
// what decides whether this banner shows at all.
export function AccessBanner({ project }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);
  const state = accessBanner(project.my_role);
  if (!state) {
    return null;
  }

  async function run(change) {
    setBusy(true);
    setError(null);
    try {
      await change(projectScoped(project.name));
      await loadProjects();
    } catch (requestError) {
      setError(requestError);
    } finally {
      setBusy(false);
    }
  }

  return html`
    <div class="access-banner" role="status">
      ${state === "pending"
        ? html`
            <span>Access requested</span>
            <button
              type="button"
              disabled=${busy}
              onClick=${() => run((api) => api.withdrawAccessRequest())}
            >
              Withdraw
            </button>
          `
        : html`
            <span>You can view this project. Request access to make changes.</span>
            <button type="button" disabled=${busy} onClick=${() => run((api) => api.requestAccess())}>
              Request access
            </button>
          `}
      ${error ? html`<span class="access-banner-error">${error.message}</span>` : null}
    </div>
  `;
}
```

- [ ] **Step 2: Write the member list**

Create `tools/filer-task-web/static/js/components/MemberList.js`:
```js
import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { isLastOwner } from "../lib/access.js";

const LAST_OWNER_NOTE = "A project needs at least one owner";

function formatTime(unixSeconds) {
  return new Date(unixSeconds * 1000).toLocaleString();
}

// Members of one project. Owners change roles and remove members here. The
// last owner's controls stay disabled, because the server refuses to leave a
// project without an owner.
export function MemberList({ projectName, canManage, reloadKey }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [members, setMembers] = useState([]);
  const [error, setError] = useState(null);
  const [busyId, setBusyId] = useState(null);

  async function load() {
    try {
      const response = await api.listMembers();
      setMembers(response.members);
      setError(null);
    } catch (requestError) {
      setError(requestError);
    }
  }

  useEffect(() => {
    load();
  }, [projectName, reloadKey]);

  async function run(member, change) {
    setBusyId(member.user_id);
    setError(null);
    try {
      await change();
      await load();
    } catch (requestError) {
      setError(requestError);
    } finally {
      setBusyId(null);
    }
  }

  function remove(member) {
    if (!confirm(`Remove ${member.username} from ${projectName}?`)) {
      return;
    }
    run(member, () => api.removeMember(member.user_id));
  }

  return html`
    <div class="member-list">
      ${error
        ? html`<p class="screen-error" role="alert">Member request failed: ${error.message}</p>`
        : null}
      ${members.length === 0 && !error ? html`<p class="empty-state">No members.</p>` : null}
      ${members.map((member) => {
        const locked = isLastOwner(member, members);
        const busy = busyId === member.user_id;
        return html`
          <div class="session-row" key=${member.user_id}>
            <div class="session-label">
              <span>${member.username}</span>
            </div>
            <div class="session-meta">
              <span>${member.granted_by ? `Granted by ${member.granted_by}` : "Granted by the server"}</span>
              <span>${formatTime(member.granted_at)}</span>
            </div>
            ${canManage
              ? html`
                  <select
                    class="member-role"
                    value=${member.role}
                    disabled=${locked || busy}
                    title=${locked ? LAST_OWNER_NOTE : ""}
                    onChange=${(event) =>
                      run(member, () => api.setMemberRole(member.user_id, event.currentTarget.value))}
                  >
                    <option value="owner">Owner</option>
                    <option value="editor">Editor</option>
                  </select>
                  <button
                    type="button"
                    class="session-revoke"
                    disabled=${locked || busy}
                    title=${locked ? LAST_OWNER_NOTE : ""}
                    onClick=${() => remove(member)}
                  >
                    ${busy ? "Working…" : "Remove"}
                  </button>
                `
              : html`<span class="session-current">${member.role}</span>`}
          </div>
        `;
      })}
    </div>
  `;
}
```

- [ ] **Step 3: Write the access request list**

Create `tools/filer-task-web/static/js/components/AccessRequests.js`:
```js
import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";

function formatTime(unixSeconds) {
  return new Date(unixSeconds * 1000).toLocaleString();
}

// Pending requests for one project, shown to owners. Approval grants the
// editor role; an owner promotes an editor from the member list afterwards.
export function AccessRequests({ projectName, onApproved }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [requests, setRequests] = useState([]);
  const [error, setError] = useState(null);
  const [busyId, setBusyId] = useState(null);

  async function load() {
    try {
      const response = await api.listAccessRequests();
      setRequests(response.requests);
      setError(null);
    } catch (requestError) {
      setError(requestError);
    }
  }

  useEffect(() => {
    load();
  }, [projectName]);

  async function run(request, change) {
    setBusyId(request.user_id);
    setError(null);
    try {
      await change();
      await load();
    } catch (requestError) {
      setError(requestError);
    } finally {
      setBusyId(null);
    }
  }

  function approve(request) {
    run(request, async () => {
      await api.setMemberRole(request.user_id, "editor");
      onApproved();
    });
  }

  return html`
    <div class="member-list">
      ${error
        ? html`<p class="screen-error" role="alert">Access request failed: ${error.message}</p>`
        : null}
      ${requests.length === 0 && !error
        ? html`<p class="empty-state">No pending requests.</p>`
        : null}
      ${requests.map(
        (request) => html`
          <div class="session-row" key=${request.user_id}>
            <div class="session-label">
              <span>${request.username}</span>
            </div>
            <div class="session-meta">
              <span>Requested ${formatTime(request.requested_at)}</span>
            </div>
            <button type="button" disabled=${busyId === request.user_id} onClick=${() => approve(request)}>
              Approve
            </button>
            <button
              type="button"
              class="session-revoke"
              disabled=${busyId === request.user_id}
              onClick=${() => run(request, () => api.denyAccessRequest(request.user_id))}
            >
              Deny
            </button>
          </div>
        `,
      )}
    </div>
  `;
}
```

- [ ] **Step 4: Replace the Settings screen**

Replace `tools/filer-task-web/static/js/screens/Settings.js` entirely:
```js
import { html, useEffect, useMemo, useRef, useState } from "../../vendor/preact-htm.js";
import { listSessions, projectScoped } from "../api/client.js";
import { AccessRequests } from "../components/AccessRequests.js";
import { Header } from "../components/Header.js";
import { MemberList } from "../components/MemberList.js";
import { PolicyDomains } from "../components/PolicyDomains.js";
import { PolicyTags } from "../components/PolicyTags.js";
import { PolicyTaskTypes } from "../components/PolicyTaskTypes.js";
import { ProjectCreateDialog } from "../components/ProjectCreateDialog.js";
import { ProjectOpenDialog } from "../components/ProjectOpenDialog.js";
import { RejectionNotice } from "../components/RejectionNotice.js";
import { SessionList } from "../components/SessionList.js";
import { ACTIONS, can } from "../lib/access.js";
import { sectionForOperation } from "../lib/policyOps.js";
import { policyRejection, sectionRejection } from "../lib/policyRejection.js";
import { openProject } from "../lib/projectOpen.js";
import { fieldError } from "../lib/rejection.js";
import { sessionRequestFailed, sessionRequestSucceeded } from "../lib/sessions.js";
import { useIdentityStore } from "../store/identity.js";
import { activeProject, loadProjects, useProjectStore } from "../store/project.js";

export function SettingsScreen({ projectName }) {
  const identityStore = useIdentityStore();
  useProjectStore();
  const api = useMemo(() => (projectName ? projectScoped(projectName) : null), [projectName]);
  const [policy, setPolicy] = useState(null);
  const [rejection, setRejection] = useState(null);
  const [busy, setBusy] = useState(false);
  const [sessionState, setSessionState] = useState(sessionRequestSucceeded([]));
  const [sessionBusy, setSessionBusy] = useState(false);
  const [armed, setArmed] = useState(null);
  const [dialog, setDialog] = useState(null);
  const [memberReload, setMemberReload] = useState(0);
  // A response that outlives a project switch would repaint this screen with
  // the policy of the project the user has already left.
  const guardRef = useRef({ cancelled: false });

  const project = activeProject();
  const role = project && project.name === projectName ? project.my_role ?? null : null;
  const isAdmin = identityStore.identity?.is_admin === true;
  const canRegister = can(role, isAdmin, ACTIONS.REGISTER_PROJECTS);
  const canEditPolicy = can(role, isAdmin, ACTIONS.EDIT_POLICY);
  const canManageMembers = can(role, isAdmin, ACTIONS.MANAGE_MEMBERS);

  async function load(guard = guardRef.current) {
    if (!api) {
      setPolicy(null);
      return;
    }
    try {
      const loaded = await api.getPolicy();
      if (!guard.cancelled) {
        setPolicy(loaded);
        setRejection(null);
      }
    } catch (error) {
      if (!guard.cancelled) {
        setPolicy(null);
        setRejection({ section: "policy", ...policyRejection(error) });
      }
    }
  }

  async function loadSessions(guard = guardRef.current) {
    setSessionBusy(true);
    try {
      const response = await listSessions();
      if (!guard.cancelled) {
        setSessionState(sessionRequestSucceeded(response.sessions));
      }
    } catch (error) {
      if (!guard.cancelled) {
        setSessionState(sessionRequestFailed([], error));
      }
    } finally {
      if (!guard.cancelled) {
        setSessionBusy(false);
      }
    }
  }

  async function refresh() {
    setMemberReload((count) => count + 1);
    await Promise.all([load(), loadSessions()]);
  }

  useEffect(() => {
    const guard = { cancelled: false };
    guardRef.current = guard;
    setArmed(null);
    load(guard);
    loadSessions(guard);
    return () => {
      guard.cancelled = true;
    };
  }, [projectName]);

  // Both dialogs report their own refusal, so the screen only has to close them
  // once the project they asked for is the active one.
  async function open(path) {
    return dismissOnSuccess(await openProject(path, false), policyRejection);
  }

  async function create(location, name) {
    return dismissOnSuccess(await openProject(location, true, name), fieldError);
  }

  function dismissOnSuccess(result, normalize) {
    if (result.ok) {
      setDialog(null);
      return { ok: true };
    }
    return { ok: false, rejection: normalize(result.error) };
  }

  // The response carries the whole refreshed policy, so an accepted change
  // needs no second read. Project summaries do go stale, because a domain
  // change moves the sidebar's counts.
  async function submit(operation) {
    if (!operation || !api) {
      return false;
    }
    const guard = guardRef.current;
    setBusy(true);
    setRejection(null);
    setArmed(null);
    try {
      const fresh = await api.patchPolicy(operation);
      if (!guard.cancelled) {
        setPolicy(fresh);
      }
      await loadProjects();
      return true;
    } catch (error) {
      if (!guard.cancelled) {
        setRejection({ section: sectionForOperation(operation), ...policyRejection(error) });
      }
      return false;
    } finally {
      setBusy(false);
    }
  }

  return html`
    <section class="screen">
      <${Header} title="Settings" onRefresh=${refresh} />
      <h3 class="settings-heading">Projects</h3>
      ${canRegister
        ? html`
            <div class="settings-actions">
              <button type="button" onClick=${() => setDialog("open")}>Open a project…</button>
              <button type="button" onClick=${() => setDialog("create")}>Create a project…</button>
            </div>
          `
        : html`<p class="muted-note">Only server admins can register projects.</p>`}
      <${RejectionNotice} rejection=${sectionRejection(rejection, "policy")} />
      ${policy && canEditPolicy
        ? html`
            <${PolicyDomains}
              policy=${policy}
              rejection=${rejection}
              armed=${armed}
              onArm=${setArmed}
              onSubmit=${submit}
              busy=${busy}
            />
            <${PolicyTaskTypes}
              policy=${policy}
              rejection=${rejection}
              onSubmit=${submit}
              busy=${busy}
            />
            <${PolicyTags} policy=${policy} rejection=${rejection} onSubmit=${submit} busy=${busy} />
          `
        : null}
      ${policy && !canEditPolicy
        ? html`<p class="muted-note">Only project owners can change domains, task types, and tags.</p>`
        : null}
      ${projectName
        ? html`
            <h3 class="settings-heading">Members</h3>
            <${MemberList}
              projectName=${projectName}
              canManage=${canManageMembers}
              reloadKey=${memberReload}
            />
          `
        : null}
      ${projectName && canManageMembers
        ? html`
            <h3 class="settings-heading">Access requests</h3>
            <${AccessRequests}
              projectName=${projectName}
              onApproved=${() => setMemberReload((count) => count + 1)}
            />
          `
        : null}
      <h3 class="settings-heading">Active sessions</h3>
      <${SessionList}
        sessions=${sessionState.sessions}
        error=${sessionState.error}
        busy=${sessionBusy}
        onRevoked=${loadSessions}
        onError=${(error) =>
          setSessionState((current) =>
            error
              ? sessionRequestFailed(current.sessions, error)
              : sessionRequestSucceeded(current.sessions),
          )}
      />
      ${dialog === "open"
        ? html`<${ProjectOpenDialog} onOpen=${open} onCancel=${() => setDialog(null)} />`
        : null}
      ${dialog === "create"
        ? html`<${ProjectCreateDialog} onCreate=${create} onCancel=${() => setDialog(null)} />`
        : null}
    </section>
  `;
}
```

- [ ] **Step 5: Show the banner and gate the screens in the app shell**

In `tools/filer-task-web/static/js/app.js`, add imports after the `TaskDrawer` import:
```js
import { AccessBanner } from "./components/AccessBanner.js";
import { ACTIONS, can } from "./lib/access.js";
```

Replace:
```js
  const active = project ? screen : "settings";
```
with:
```js
  const canWrite = can(
    project?.my_role ?? null,
    identityStore.identity?.is_admin === true,
    ACTIONS.WRITE_TASKS,
  );
  // New task is a write screen; a user who cannot write, or has just lost the
  // role, lands on Ready instead.
  const active = !project ? "settings" : screen === "new-task" && !canWrite ? "ready" : screen;
```

Replace the `<main class="app-main">` block:
```js
      <main class="app-main">
        ${project && !project.broken && active !== "settings"
          ? html`<${AccessBanner} project=${project} />`
          : null}
        ${project && project.broken && active !== "settings"
          ? html`<${BrokenScreen}
              project=${project}
              onSwitchProject=${() => setPaletteOpen(true)}
              onOpenSettings=${() => setScreen("settings")}
            />`
          : html`<${Screen} screen=${active} project=${project} onSelectTask=${setSelectedTaskId} />`}
      </main>
      <${TaskDrawer}
        projectName=${project?.name}
        taskId=${selectedTaskId}
        canWrite=${canWrite}
        onClose=${() => setSelectedTaskId(null)}
        onSelect=${setSelectedTaskId}
      />
```

- [ ] **Step 6: Hide New task in the sidebar**

In `tools/filer-task-web/static/js/components/Sidebar.js`, add imports:
```js
import { ACTIONS, can } from "../lib/access.js";
import { useIdentityStore } from "../store/identity.js";
```
After `const project = activeProject();` add:
```js
  const identity = useIdentityStore().identity;
  const canWrite = can(project?.my_role ?? null, identity?.is_admin === true, ACTIONS.WRITE_TASKS);
  const navItems = NAV_ITEMS.filter((item) => item.id !== "new-task" || canWrite);
```
and change `${NAV_ITEMS.map(` to `${navItems.map(`.

- [ ] **Step 7: Gate the drawer**

In `tools/filer-task-web/static/js/components/TaskDrawer.js`:
- Change the signature to `export function TaskDrawer({ projectName, taskId, canWrite, onClose, onSelect }) {`.
- In the `<${DrawerBody}` props, add `canWrite=${canWrite}` after `context=${context}`.
- In `function DrawerBody({`, add `canWrite,` after `context,`.
- Change `if (draft) {` to `if (draft && canWrite) {`.
- Replace:
```js
    <div class="drawer-action-row">
      <${DrawerActions} pendingAction=${pendingAction} onRun=${onRun} />
      <button class="drawer-edit-open" onClick=${onEdit}>Edit</button>
    </div>
```
with:
```js
    ${canWrite
      ? html`
          <div class="drawer-action-row">
            <${DrawerActions} pendingAction=${pendingAction} onRun=${onRun} />
            <button class="drawer-edit-open" onClick=${onEdit}>Edit</button>
          </div>
        `
      : null}
```
- In the `<${DrawerCriteria}` props, add `readOnly=${!canWrite}`.

In `tools/filer-task-web/static/js/components/DrawerCriteria.js`, change the signature to `export function DrawerCriteria({ heading, criteria, refusal, pendingIndex, readOnly, onToggle }) {` and the checkbox's `disabled=${pendingIndex !== null}` to `disabled=${readOnly || pendingIndex !== null}`.

- [ ] **Step 8: Gate the palette and label the activity**

In `tools/filer-task-web/static/js/components/CommandPalette.js`, add `import { useIdentityStore } from "../store/identity.js";` and replace `const rows = paletteRows(store.projects, query);` with:
```js
  const identity = useIdentityStore().identity;
  const rows = paletteRows(store.projects, query, { canCreate: identity?.is_admin === true });
```

In `tools/filer-task-web/static/js/screens/Activity.js`, add `import { activityLabel } from "../lib/activity.js";` and change `<td>${row.action}</td>` to `<td>${activityLabel(row.action)}</td>`.

- [ ] **Step 9: Style the banner and member controls**

Append to `tools/filer-task-web/static/style.css`:
```css
.access-banner {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 0 0 12px;
  padding: 10px 12px;
  background: var(--panel);
  border: 1px solid var(--accent);
  border-radius: 8px;
}

.access-banner-error {
  color: var(--danger-text);
}

.member-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: 640px;
}

.member-role {
  background: var(--bg);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 4px 6px;
}
```

- [ ] **Step 10: Run the JS and crate tests**

Run: `cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu`
Expected: PASS.

- [ ] **Step 11: Check the controls in a browser**

Create a scratch project and start the server on a scratch database:
```bash
SCRATCH="$(mktemp -d)"
mkdir -p "$SCRATCH/demo/.tasks/core"
printf -- '---\nid: CORE-001\ntitle: Demo task\nstatus: To Do\npriority: High\ntype: Feature\n---\n\n## Acceptance Criteria\n\n- [ ] Works\n' > "$SCRATCH/demo/.tasks/core/CORE-001-demo-task.md"
echo "$SCRATCH"
cargo +stable-x86_64-pc-windows-gnu run --locked -p filer-task-web --target x86_64-pc-windows-gnu -- --port 7979 --database "$SCRATCH/state.sqlite3"
```
Open `http://127.0.0.1:7979` in a normal window and a private window, then confirm each item:
1. Normal window: pick "Olivia". Settings shows Open and Create; open `<SCRATCH>/demo`. Members lists Olivia as owner with disabled controls.
2. Private window: pick "Sam". The banner reads "You can view this project. Request access to make changes."; New task is missing from the sidebar; the drawer for CORE-001 has no action row and disabled checkboxes; Settings says "Only server admins can register projects." and "Only project owners can change domains, task types, and tags."; Ctrl+K with an unknown name offers no create row.
3. Private window: Request access. The banner changes to "Access requested".
4. Normal window: Refresh Settings. Access requests lists Sam; Approve. Members lists Sam as editor.
5. Private window: Refresh. The banner disappears; New task and the drawer's actions appear; Start CORE-001 works.
6. Normal window: Remove Sam. Private window: press Done on CORE-001. The drawer shows the refusal, and the actions disappear after the role reload.
7. Normal window: Activity shows "requested access", "granted access", and "removed member".

Stop the server with Ctrl+C.

- [ ] **Step 12: Commit**

```bash
git add tools/filer-task-web/static
git commit -m "feat(task-web): show membership controls in the browser"
```

---

### Task 10: README, task completion, and final verification

**Files:**
- Modify: `tools/filer-task-web/README.md`
- Modify (by command): `.tasks/web/WEB-032-require-project-membership-for-writes.md`

**Interfaces:**
- Consumes: the behavior of Tasks 1 to 9.
- Produces: user documentation and a `Done` WEB-032.

- [ ] **Step 1: Update the run section's network warning**

In `tools/filer-task-web/README.md`, replace:
```
A non-loopback host lets other machines reach the board. Anyone who reaches it can pick a username, change tasks, and register or create a project at any path the server process can write. Listen on a shared address only on a network you trust.
```
with:
```
A non-loopback host lets other machines reach the board, and the server prints a warning when it starts on one. Anyone who reaches it can read every project. Changing a project needs membership in it, and registering a project needs server admin; see [Projects and members](#projects-and-members). The board speaks plain HTTP, so session cookies cross the network unencrypted. Listen on a shared address only on a network you trust, or behind a reverse proxy that terminates TLS.
```

If this paragraph's text differs from the old text above, open the README, find the paragraph after the `--host` example that describes non-loopback hosts, and replace that paragraph.

- [ ] **Step 2: Update the project, board, identity, and storage sections**

Replace `The board shows no tasks until you register a project. In Settings, choose "Open a project…" and enter any directory inside a project;` with `The board shows no tasks until a server admin registers a project. In Settings, an admin chooses "Open a project…" and enters any directory inside a project;`.

Replace `Choose "Create a project…" to run` with `An admin can instead choose "Create a project…" to run`.

Replace `- Settings registers projects, edits the project's domains, tags, and task types, and lists your browser sessions.` with `- Settings registers projects, edits the project's domains, tags, and task types, manages members and access requests, and lists your browser sessions.`

Replace `A write without a valid session fails with `401 Unauthorized`, before the server touches any task file.` with `A write without a valid session fails with `401 Unauthorized`, and a write by a user without the role it needs fails with `403 Forbidden`, both before the server touches any task file.`

Replace `The SQLite database holds project registrations, users, sessions, pairing codes, and the activity log.` with `The SQLite database holds project registrations, users, sessions, pairing codes, project members, access requests, and the activity log.`

- [ ] **Step 3: Add the Projects and members section**

Insert this section directly before the `## Concurrent writes` heading:

~~~markdown
## Projects and members

Each project has members, and each member is an owner or an editor. Anyone who can reach the board reads every project, but changing a project needs membership in it:

| Action | Editor | Owner |
|---|---|---|
| Create, edit, check off, and transition tasks | yes | yes |
| Change the project's domains, tags, and task types | no | yes |
| Approve, deny, change, and remove members | no | yes |
| Remove the project registration | no | yes |

A user who is not a member sees a banner on the project's screens with a Request access button. Owners review requests under Access requests in Settings. Approval makes the user an editor, and an owner can then promote them in Members. A project always keeps at least one owner, so the board refuses to remove or demote the last one.

Registering a project needs server admin, and the admin who registers a project becomes its owner. The first identity created on an empty database becomes admin. Other admins, and any role on any project, come from the command line:

```bash
cargo run -p filer-task-web -- admin-grant <username>
cargo run -p filer-task-web -- admin-revoke <username>
cargo run -p filer-task-web -- member-grant <project> <username> --role owner
cargo run -p filer-task-web -- member-revoke <project> <username>
cargo run -p filer-task-web -- member-list <project>
```

The command line is the operator override: it can remove a project's last owner or the last admin, and it prints a warning when it does. Each command accepts `--database`, which must name the file the server uses.
~~~

- [ ] **Step 4: Add the routes to the API table**

In the `## HTTP API` table, insert these rows after the `/api/projects/{project}/policy` row:
```
| `/api/projects/{project}/members` | GET | List members and their roles |
| `/api/projects/{project}/members/{user_id}` | PUT, DELETE | Grant or change a role, revoke a member |
| `/api/projects/{project}/access-requests` | GET | List pending access requests |
| `/api/projects/{project}/access-requests/{user_id}` | DELETE | Deny an access request |
| `/api/projects/{project}/access-request` | POST, DELETE | Request or withdraw access for yourself |
```

After the paragraph that follows the table (the one starting `The context endpoint is separate`), add:
```
Task writes need the editor or owner role. Policy changes, member management, and removing a registration need the owner role, and registering a project needs server admin. A refused request fails with `403` and the code `project_role_required` or `admin_required`. `GET /api/projects` reports your role on each project as `my_role`, and the identity routes report `is_admin`.
```

- [ ] **Step 5: Run the full verification**

Run each and confirm the expected result:
```bash
cargo +stable-x86_64-pc-windows-gnu fmt --all --check
```
Expected: no output.
```bash
cargo +stable-x86_64-pc-windows-gnu clippy --locked --workspace --exclude filer-app --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings
```
Expected: no warnings.
```bash
cargo +stable-x86_64-pc-windows-gnu test --locked -p filer-task-web --target x86_64-pc-windows-gnu
```
Expected: PASS, every test file including `membership_storage_test`, `membership_api_test`, `membership_cli_test`, and `frontend_js_test`.

- [ ] **Step 6: Check every criterion and mark WEB-032 done**

Toggle each of the 15 acceptance criteria (zero-based indexes 0 to 14):
```bash
for index in $(seq 0 14); do
  cargo +stable-x86_64-pc-windows-gnu run -q --locked -p taskroot --target x86_64-pc-windows-gnu -- criterion-toggle web:WEB-032 "$index"
done
cargo +stable-x86_64-pc-windows-gnu run -q --locked -p taskroot --target x86_64-pc-windows-gnu -- done web:WEB-032
cargo +stable-x86_64-pc-windows-gnu run -q --locked -p taskroot --target x86_64-pc-windows-gnu -- validate
```
Expected: `Task Done` for WEB-032, then `Status: Passed` with `Warnings: 0`. Open the task file and confirm all 15 boxes read `- [x]`.

- [ ] **Step 7: Commit**

```bash
git add tools/filer-task-web/README.md .tasks/web/WEB-032-require-project-membership-for-writes.md
git commit -m "docs(task-web): document project membership and complete WEB-032"
```
