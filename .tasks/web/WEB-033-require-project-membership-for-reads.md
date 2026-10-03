---
id: "WEB-033"
title: "Require project membership for reads"
status: Deferred
priority: "Low"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-032"]
risk: "Medium"
impact: "Keeps task content private to a project's members when the board listens on a network shared with people outside the team."
tags: ["web", "server", "api"]
last_updated: 2026-10-03
---

## Summary

web:WEB-032 gates writes behind project membership but leaves every read open, so anyone who can reach the board sees every registered project, task, member list, and activity row. Require membership to read a project's tasks, milestones, policy, members, and activity. A non-member sees only the project name in the project list and the request-access action. Reads without a session fail with 401, and reads by a signed-in non-member fail with 403. The `ProjectEditor` and `ProjectOwner` extractors from web:WEB-032 extend with a `ProjectMember` guard for read routes, and the route table test grows to cover every GET route.

## Rationale

The board targets a small trusted LAN team whose task content is not secret, so web:WEB-032 keeps reads open to avoid changing every read route and the project list in the same change. Pick this up when the board needs to run on a network shared with people outside the team.
