# Project Membership for filer-task-web

`filer-task-web` records who made each write, but it lets anyone with a session change any registered project. On the loopback default that is one person's machine, so attribution is enough. With `--host`, anyone on the network can pick a username, edit every project, and register a project at any path the server process can write. This design adds per-project membership: a user must be approved for a project before they can change it, and only server admins can register projects. Reads stay open.

Task: web:WEB-032. Deferred follow-up for gated reads: web:WEB-033.

## Constraints

- Identity stays passwordless. Sessions, pairing PINs, and the recovery CLI from web:WEB-030 and web:WEB-031 keep working unchanged.
- The `.tasks/` files remain the only source of truth for task content. Membership lives in the SQLite database and never in task files.
- Existing databases migrate in place. A person who has written to a project keeps write access after the upgrade.
- The server operator keeps full control through the CLI, following the recovery rule: whoever can run the CLI controls the server.
- The target is a small trusted LAN team. The design does not defend against an attacker who can read LAN traffic; there is no TLS.

## Roles and permissions

A user holds at most one role per project: owner or editor. A user may also hold a pending access request for a project they are not a member of. Separately, a user may be a server admin. Admin controls registration only; on a project, an admin has exactly the permissions of their membership there. The admin flag changes only through the CLI or the first-identity rule; no web route grants or revokes it.

| Action | Non-member | Pending | Editor | Owner | Admin without membership |
|---|---|---|---|---|---|
| Read projects, tasks, members, and activity | yes | yes | yes | yes | yes |
| Request access | yes | already pending | no | no | yes |
| Create, edit, check criteria, and transition tasks | no | no | yes | yes | no |
| Approve, deny, or revoke members, and change roles | no | no | no | yes | no |
| Change project policy | no | no | no | yes | no |
| Remove the project registration | no | no | no | yes | no |
| Register a project and become its owner | no | no | no | no | yes |

Rules that apply across the table:

- Every project keeps at least one owner. The web API refuses to remove or demote the last owner. Only the CLI can.
- A denied request is deleted, so the user can request again. There are no notifications; owners see pending requests in Settings.
- A request without a session fails with 401. A request with a session but without the required role fails with 403.

## Data model

Migration `0007_project_membership.sql` adds an admin flag and two tables. The connection enables foreign keys, so the cascades below apply.

```sql
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
```

Pending requests live in their own table so that `project_members` holds only users who have access, and a membership check is one primary-key lookup with no status filter. `granted_by` is NULL when the migration or the CLI granted the role.

Both tables key projects by name because the registry and the `activity` table already address projects by name, and `project_registrations.name` is unique. Removing a registration deletes its members and requests, so registering the same name later starts with no members.

## Migration backfill

The same migration backfills existing data in this order:

1. The user with the lowest id becomes admin. That user is the first identity ever created, which matches the rule for fresh databases.
2. Each registered project gets as owner the user on its most recent `project.register` activity row. The backfill joins `users` and `project_registrations`, so activity rows for removed projects or missing users are skipped.
3. A registered project with no such row gets the admin from step 1 as owner.
4. Every other user with any activity row in a project becomes an editor there (`INSERT OR IGNORE`, so owners keep the owner role).

A database with registered projects and no users has no admin to assign. Those projects stay ownerless until an operator runs `member-grant`.

## Invariants

- The first identity on a database with no users becomes admin. `create_identity` and the recovery CLI's user creation set `is_admin` to `NOT EXISTS (SELECT 1 FROM users)` inside the insert statement. SQLite serializes writes, so two browsers onboarding at once produce exactly one admin.
- Registering a project and inserting its owner row happen in one transaction. A crash cannot leave a registered project without an owner.
- Approving a request inserts the member and deletes the request in one transaction.
- Removing or demoting the last owner is refused inside the transaction that would perform it, so two concurrent demotions cannot both succeed. The CLI bypasses this check deliberately and prints a warning.

## Enforcement

`src/access.rs` defines three Axum extractors built on the existing `Actor`:

- `ServerAdmin` resolves the session and requires `users.is_admin`.
- `ProjectEditor` resolves the session, reads the `project` path parameter with `RawPathParams`, and requires the editor or owner role.
- `ProjectOwner` does the same and requires the owner role.

`ProjectEditor` and `ProjectOwner` share one helper, `require_project_role(parts, state, Role)`. Each extractor checks in this order: a missing or unknown session fails with 401 `identity_required`, an unregistered project fails with 404, and a missing role fails with 403. Reads are open, so the 404 reveals nothing a read would not.

The extractors run before body extractors, as `Actor` does. A forbidden request is rejected before its JSON body is parsed and before the handler takes the project's write lock.

| Guard | Routes |
|---|---|
| `ProjectEditor` | `POST /api/projects/{project}/tasks`, `PATCH /api/projects/{project}/tasks/{id}`, `PUT /api/projects/{project}/tasks/{id}/criteria/{index}`, `POST /api/projects/{project}/tasks/{id}/{start,done,block,defer,obsolete}` |
| `ProjectOwner` | `PATCH /api/projects/{project}/policy`, `DELETE /api/projects/{project}`, `PUT` and `DELETE /api/projects/{project}/members/{user_id}`, `GET /api/projects/{project}/access-requests`, `DELETE /api/projects/{project}/access-requests/{user_id}` |
| `ServerAdmin` | `POST /api/projects` |
| `Actor` | identity, pairing, and session routes; `POST` and `DELETE /api/projects/{project}/access-request` |
| none | every other `GET` |

`write::mutate` takes a `ProjectEditor` instead of a separate `Actor` and project name, so the project a write changes is always the project its role was checked against.

The role check runs before the write lock. If an owner revokes a member while that member's write is in flight, that one write can complete. Re-checking under the lock would close the window, but the cost is not justified for a trusted-LAN tool.

## HTTP API

| Path | Method | Guard | Behavior |
|---|---|---|---|
| `/api/projects/{project}/members` | GET | none | Members with user id, username, role, granted-by, and granted-at |
| `/api/projects/{project}/members/{user_id}` | PUT | owner | Body `{ "role": "owner" \| "editor" }`. Adds the member or changes the role. Deletes that user's pending request in the same transaction. Fails with 404 `user_not_found` for an unknown user id |
| `/api/projects/{project}/members/{user_id}` | DELETE | owner | Revokes the member. Fails with 409 `last_owner` for the last owner |
| `/api/projects/{project}/access-requests` | GET | owner | Pending requests with user id, username, and requested-at |
| `/api/projects/{project}/access-requests/{user_id}` | DELETE | owner | Denies the request by deleting it. Fails with 404 `access_request_not_found` when none exists |
| `/api/projects/{project}/access-request` | POST | session | Requests access for the acting user. Repeating it while pending succeeds without change. Fails with 409 `already_member` for members |
| `/api/projects/{project}/access-request` | DELETE | session | Withdraws the acting user's pending request |

Paths address users by id because ids survive a rename and the UI already holds them from list responses.

Existing responses gain two fields. `GET /api/projects` adds `my_role` to each summary: `owner`, `editor`, `pending`, or `null`. The route stays open and fills the field only when the request carries a valid session. `GET /api/identity` adds `is_admin`. These fields let the UI decide which controls to show without extra requests.

`WebError` gains these variants, using the existing JSON error shape with a `code` field:

| Variant | Status | Code |
|---|---|---|
| `AdminRequired` | 403 | `admin_required` |
| `ProjectRoleRequired` | 403 | `project_role_required`, with the required role in the message |
| `LastOwner` | 409 | `last_owner` |
| `AlreadyMember` | 409 | `already_member` |
| `AccessRequestNotFound` | 404 | `access_request_not_found` |
| `UserNotFound` | 404 | `user_not_found` |

Membership changes made through the web write activity rows with no task id: `member.grant`, `member.role`, `member.revoke`, `access.request`, and `access.deny`. The detail field names the affected user and role.

## CLI

The recovery subcommands set the style: a positional username and an optional `--database`.

```
filer-task-web admin-grant <username> [--database <path>]
filer-task-web admin-revoke <username> [--database <path>]
filer-task-web member-grant <project> <username> --role owner|editor [--database <path>]
filer-task-web member-revoke <project> <username> [--database <path>]
filer-task-web member-list <project> [--database <path>]
```

The CLI is the operator override. It may revoke the last owner of a project or the last admin, and prints a warning to stderr when it does. `member-grant` deletes the user's pending request for that project. CLI commands write no activity rows because they run without a web session, matching `session-mint`; each command prints what it changed.

When `--host` names a non-loopback address, the server prints this warning to stderr at startup: reads are open to anyone who can reach this address; writes need project membership.

## Browser UI

Permission rules live in one pure function, `can(role, isAdmin, action)`, in `static/js/lib/access.js`. Components call it instead of comparing roles. `store/identity.js` holds `isAdmin` from `GET /api/identity`, and `store/project.js` holds `myRole` from the project summaries.

When the user cannot write to the open project, the UI hides the New task sidebar item, the transition buttons in `DrawerActions`, the form in `DrawerEdit`, the checkboxes in `DrawerCriteria`, and the write commands in the command palette. A banner above the project screens reads "You can view this project. Request access to make changes." with a Request access button. While the request is pending, the banner reads "Access requested" with a Withdraw button.

Settings gains two sections:

- Members (`components/MemberList.js`) lists members for everyone. Owners get a role select and a Remove button on each row, disabled for the last owner with an explanation.
- Access requests (`components/AccessRequests.js`) is visible to owners only. Each request has Approve, which grants the editor role, and Deny. Owners promote an editor to owner from Members.

Only admins see the project open and create dialogs. Other users see "Only server admins can register projects." Only owners see the action that removes a registration.

`api/client.js` and `lib/rejection.js` map `project_role_required`, `admin_required`, and `last_owner` onto the existing rejection notice. A `project_role_required` rejection refreshes `myRole`, so controls disappear after an owner revokes the user mid-session. The Activity screen renders the membership actions as readable sentences, such as "approved Minh as editor".

## Testing

Storage tests in `tests/membership_storage_test.rs`, following the migration test pattern from web:WEB-030:

- Backfill: lowest-id admin, owner from the latest `project.register` row, admin fallback owner, editors from activity, stale activity rows skipped, and a database with no users leaving projects ownerless.
- First identity becomes admin, and two identities created at once on an empty database produce exactly one admin.
- Registration inserts the owner row atomically, approval deletes the request atomically, and removing a registration cascades to members and requests.
- Removing or demoting the last owner is refused.

API tests in `tests/membership_api_test.rs`:

- A route table lists every non-GET route with its guard. Each route guarded by `ServerAdmin`, `ProjectEditor`, or `ProjectOwner` returns 401 without a session and 403 for a signed-in non-member, and the project's `.tasks/` files are byte-identical afterwards. Routes guarded only by `Actor` are listed with their guard and return 401 without a session. A comment beside `app::router` requires every new non-GET route to join this table.
- Editors can write tasks and receive 403 on owner routes. Owners manage members and requests. Only admins register projects.
- `my_role` and `is_admin` report the correct values, and each membership change records its activity row.

CLI tests use the shared runner in `tests/cli`: grant and revoke for admins and members, `member-list` output, the last-owner warning, and the startup warning appearing on stderr for a non-loopback host and not for loopback.

Frontend tests: `tests/js/access.test.js` checks `can()` against every row of the permissions table, and `tests/js/rejection.test.js` covers the three rejection codes.

Existing tests that write as a second user need membership. A `grant_member` test helper covers them. Tests that write as the first user keep passing, because that user is admin on a fresh database and owns the projects they register.

## Out of scope

- Requiring membership for reads. web:WEB-033 tracks this as deferred work.
- Leaving a project yourself. Owners and the CLI remove members.
- Notifications for access requests.
- TLS. The README tells operators to use a trusted network or a TLS reverse proxy.
- Re-checking the role under the write lock.
- Activity rows for CLI actions.

## Rejected alternatives

Gating reads in this task. Requiring membership to read would change every read route and the project list. The target is a trusted LAN team whose task content is not secret, so reads stay open and web:WEB-033 holds the work.

Letting every signed-in user register projects. Registration can create a directory and initialize `.tasks/` at any path the server can write. Gating task writes while leaving registration open would leave the largest hole.

Registering projects only through the CLI. It is the tightest option, but it removes the project open and create dialogs from web:WEB-027 through web:WEB-029 and forces a CLI step for solo use.

Making admins owners of every project. It is convenient, but it turns admin into a web superuser. Admins own what they register, and the CLI covers emergencies.

Checking roles inside handlers. A `require_role` call at the top of each handler changes fewer signatures, but it runs after body parsing, so a malformed body returns 400 before a forbidden request returns 403, and a new handler can omit the call with no visible sign.

A router-level middleware layer. It centralizes the checks but duplicates the route table as string patterns that drift silently when a route changes.

One members table with a status column. Holding pending requests in `project_members` would force every membership check to filter by status and would allow a pending row to carry a role it was never granted.
