# filer-task-web

`filer-task-web` serves a browser board for `taskroot` projects on your own machine. The `taskroot` CLI suits agents and scripts, but reviewing a backlog, reading acceptance criteria, and moving tasks through their lifecycle is easier with filters and a detail view. The board reads and writes the same `.tasks/` files through the `taskroot` library, so a change made in the browser passes the same validation as a change made with the CLI.

## Run the board

```bash
cargo run -p filer-task-web
```

By default the server listens on `http://127.0.0.1:7878` and keeps its state in `filer-task-web.sqlite3` in the working directory. The loopback default keeps the board on your machine. Pass `--host`, `--port`, or `--database` to change a default:

```bash
cargo run -p filer-task-web -- --port 8080 --database ~/filer-task-web.sqlite3
cargo run -p filer-task-web -- --host 0.0.0.0
```

`--host` takes an IP address such as `::1` or `0.0.0.0`, not a hostname. `--port 0` lets the OS pick a free port, and the startup line prints the address the server bound.

A non-loopback host lets other machines reach the board, and the server prints a warning when it starts on one. Anyone who reaches it can read every project. Changing a project needs membership in it, and registering a project needs server admin; see [Projects and members](#projects-and-members). The board speaks plain HTTP, so session cookies cross the network unencrypted. Listen on a shared address only on a network you trust, or behind a reverse proxy that terminates TLS.

The server reads the page, stylesheet, and scripts from this crate's `static/` directory, at the path recorded when the binary was compiled. Run it from a checkout of this repository. A binary copied to another machine starts, but answers every page request with 404.

The frontend uses Preact and htm from `static/vendor/`, so it has no build step and no `npm install`.

## Open a project

The board shows no tasks until a server admin registers a project. In Settings, an admin chooses "Open a project…" and enters any directory inside a project; the server walks up to the nearest ancestor that contains `.tasks/`. An admin can instead choose "Create a project…" to run `taskroot init` in a new or existing directory. Registrations live in the database and survive a restart.

The server opens each project from disk on every request instead of caching task files. Edits made with the CLI, an editor, or `git pull` appear on the next page load. When a project's `.tasks/` tree fails validation, the board lists the issues in place of the task screens until you fix the files.

## Use the board

The sidebar switches between projects and screens:

- Ready lists the same ready queue as `taskroot ready`.
- Tasks lists every task, with filter chips and sortable columns.
- Milestones shows each milestone's criteria, how many of its tasks are done, and its tasks grouped by status.
- New task creates a task under the same validation as `taskroot add`.
- Activity lists who changed what, newest first.
- Settings registers projects, edits the project's domains, tags, and task types, manages members and access requests, and lists your browser sessions.

Selecting a task opens a drawer with its detail, acceptance criteria, relations, and readiness blockers. From the drawer you edit fields, check criteria, and start, finish, block, defer, or obsolete the task. Press Ctrl+K, or Cmd+K on macOS, to open the command palette.

## Identity and sessions

The board records which user made each write. On your first visit it asks for a username and stores a session in an `HttpOnly` cookie that lasts 365 days. Reads need no session. A write without a valid session fails with `401 Unauthorized`, and a write by a user without the role it needs fails with `403 Forbidden`, both before the server touches any task file.

To use the same identity in a second browser, choose "Pair another browser" in the sidebar footer of the first one. In the second browser, choose "I already have a name" and enter your username with the code. The six-digit code expires after 5 minutes, works once, and locks after 5 failed attempts. Settings lists your active sessions and revokes any of them.

If you lose every cookie, mint a session from the command line:

```bash
cargo run -p filer-task-web -- session-mint <username>
cargo run -p filer-task-web -- session-clear <username>
```

`session-mint` prints a new session token and creates the user if needed. Set the token as the `filer_task_identity` cookie for the board's address in your browser's developer tools. `session-clear` revokes every session the user holds and prints how many it removed. Both commands accept `--database`, which must name the file the server uses.

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

## Concurrent writes

Each project runs its web writes one at a time behind a single lock, and `taskroot` takes an operating-system lock on `.tasks/.taskroot.lock` for every mutation. A browser write and a CLI command therefore never interleave on disk. The server reloads the project after it takes the lock, so validation sees edits made outside the board.

## Storage

The SQLite database holds project registrations, users, sessions, pairing codes, project members, access requests, and the activity log. Task content stays in each project's `.tasks/` directory, so deleting the database loses registrations and history but no tasks.

The database runs in WAL mode with `synchronous=FULL`, so a committed row survives a power loss. The migrations in `migrations/` are embedded in the binary and run when the server opens the database.

A task write commits to disk before the server records its activity row. If recording fails, the write still succeeds and the server logs the error, because the task file has already changed.

## HTTP API

The browser talks to a JSON API under `/api`. `app::router` builds the API without the static file fallback, so tests drive it directly.

| Path | Methods | Purpose |
|---|---|---|
| `/api/projects` | GET, POST | List projects, register or create one |
| `/api/projects/{project}` | DELETE | Remove a registration and keep the files |
| `/api/projects/{project}/policy` | GET, PATCH | Read and change domains, tags, and task types |
| `/api/projects/{project}/members` | GET | List members and their roles |
| `/api/projects/{project}/members/{user_id}` | PUT, DELETE | Grant or change a role, revoke a member |
| `/api/projects/{project}/access-requests` | GET | List pending access requests |
| `/api/projects/{project}/access-requests/{user_id}` | DELETE | Deny an access request |
| `/api/projects/{project}/access-request` | POST, DELETE | Request or withdraw access for yourself |
| `/api/projects/{project}/ready` | GET | Ready queue |
| `/api/projects/{project}/milestones` | GET | Per-milestone progress |
| `/api/projects/{project}/tasks` | GET, POST | List tasks, create a task |
| `/api/projects/{project}/tasks/{id}` | GET, PATCH | Read a task, edit its fields |
| `/api/projects/{project}/tasks/{id}/context` | GET | Relations, ancestors, and readiness blockers |
| `/api/projects/{project}/tasks/{id}/criteria/{index}` | PUT | Check or uncheck one criterion |
| `/api/projects/{project}/tasks/{id}/{transition}` | POST | `start`, `done`, `block`, `defer`, or `obsolete` |
| `/api/activity` | GET | Write history, filtered by `project` and `task_id` |
| `/api/identity` | GET, PUT | Read or set the session's username |
| `/api/identity/pin` | POST | Mint a pairing code |
| `/api/identity/pair` | POST | Redeem a pairing code |
| `/api/sessions` | GET | List your sessions |
| `/api/sessions/{id}` | DELETE | Revoke a session |

The context endpoint is separate from the task read because building relations costs a full task graph, and list views should not pay for it. `/api/activity` returns 50 rows by default and at most 200 per request.

Task writes need the editor or owner role. Policy changes, member management, and removing a registration need the owner role, and registering a project needs server admin. A refused request fails with `403` and the code `project_role_required` or `admin_required`. `GET /api/projects` reports your role on each project as `my_role`, and the identity routes report `is_admin`.

## Test

```bash
cargo test -p filer-task-web
```

The integration tests in `tests/` send requests to the router against temporary projects and databases. `tests/frontend_js_test.rs` also runs the browser modules in `tests/js/` through Node's built-in test runner, which needs Node.js 21 or later for its glob argument. Without Node on `PATH`, that test skips instead of failing.

## Learn more

- The [taskroot README](../taskroot/README.md) covers the task format and CLI.
- The [task tracking guide](../../docs/task-tracking.md) is the reference for frontmatter, lifecycle, and policy.
