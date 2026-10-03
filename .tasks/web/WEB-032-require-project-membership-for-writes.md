---
id: "WEB-032"
title: "Require project membership for writes"
status: "To Do"
priority: "High"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-030", "WEB-031"]
risk: "High"
impact: "Lets the board listen on a shared network without letting every visitor change every project, by requiring per-project membership for writes and server admin for registration."
tags: ["web", "server", "api", "state"]
last_updated: 2026-10-03
---

## Summary

Identity is attribution-only: anyone with a session can change any registered project, and with `--host` anyone on the network can pick a username, edit every project, and register a project at any path the server process can write. Add per-project membership with two roles, owner and editor, plus a server admin flag. Task writes need the editor role, member management, policy changes, and removing a registration need the owner role, and registering a project needs admin. Reads stay open; web:WEB-033 holds the deferred work to gate them.

Whoever registers a project becomes its owner, the first identity on an empty database becomes admin, and server CLI commands grant and revoke any role as the operator override. A migration backfills existing data so nobody who has written to a project loses access. Roles are enforced by Axum extractors that reject a request before its body is parsed or the project write lock is taken. The design lives in `docs/superpowers/specs/2026-10-03-web-032-project-membership-design.md`.

## Acceptance Criteria

- [ ] Migration 0007 adds `users.is_admin`, a `project_members` table holding owner and editor roles, and an `access_requests` table, both keyed by project name and deleted with the project registration.
- [ ] The migration makes the lowest-id user admin, makes each project's latest `project.register` actor its owner with the admin as fallback, and makes every other user with activity in a project an editor there.
- [ ] The first identity created on a database with no users becomes admin, and two identities created at once on an empty database produce exactly one admin.
- [ ] Registering a project requires admin and inserts the registrant as owner in the same transaction as the registration.
- [ ] Task creation, edits, criterion changes, and transitions require the editor or owner role; policy changes, member management, request review, and removing a registration require the owner role.
- [ ] A write without a session fails with 401, and a write with a session but without the required role fails with 403 before the request body is parsed and before any task file changes.
- [ ] Users can request and withdraw access; owners can list, approve, and deny requests, add members, change roles, and revoke members; approval deletes the request in the same transaction.
- [ ] The web API refuses to remove or demote a project's last owner with a 409 `last_owner` error.
- [ ] `GET /api/projects` reports `my_role` for a request with a session, and `GET /api/identity` reports `is_admin`.
- [ ] Membership changes made through the web record `member.grant`, `member.role`, `member.revoke`, `access.request`, and `access.deny` activity rows.
- [ ] CLI commands `admin-grant`, `admin-revoke`, `member-grant`, `member-revoke`, and `member-list` manage roles, may remove the last owner or admin with a warning, and `member-grant` deletes a pending request.
- [ ] Serving on a non-loopback `--host` prints a warning to stderr that reads are open and writes need project membership.
- [ ] The UI hides write controls when the user cannot write, shows a request-access banner, adds Members and Access requests sections to Settings, limits project open and create to admins, and refreshes the role after a `project_role_required` rejection.
- [ ] Tests cover the migration backfill, first-admin creation, atomic registration and approval, the last-owner refusal, a route table asserting 401 and 403 with unchanged files for every role-guarded route, the CLI commands and startup warning, and `can()` against the permissions table.
- [ ] The README documents roles, the access request flow, the member API routes, the CLI commands, and the network warning.
