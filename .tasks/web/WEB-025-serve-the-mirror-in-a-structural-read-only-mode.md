---
id: "WEB-025"
title: "Serve the mirror in a structural read-only mode"
status: "To Do"
priority: "High"
type: "Feature"
parent: "WEB-022"
depends_on: ["WEB-024"]
risk: "Medium"
impact: "Makes read-only structural rather than a runtime check per handler, so a future write route cannot reach the mirror by being forgotten, and keeps a writable server on loopback."
tags: ["web", "server", "api", "validation"]
last_updated: "2026-07-26"
---

## Summary

The mirror must serve every read screen and expose no write path. Split the router into a shared set of read routes and a write layer the mirror omits, so read-only is a property of how the router is built instead of a check repeated across handlers. Add GET /api/capabilities reporting whether the instance accepts writes and when each project last published. The frontend reads it through the existing policy helper and hides the New Task screen, drawer field edits, criteria toggles, and palette write commands when writes are unavailable, and the header shows the publish timestamp per project so a stale board is not mistaken for a live one. The binary binds 127.0.0.1 today and the mirror needs a bind option to serve a network, so honor that option only together with read-only mode. A writable instance then keeps its current guarantee of never leaving loopback.

## Acceptance Criteria

- [ ] Read-only mode builds a router with no write route present; every write path returns not found rather than a runtime rejection.
- [ ] The capabilities endpoint reports writability and the last publish time for each project.
- [ ] The frontend hides task creation, field edits, criteria toggles, and palette write commands when writes are unavailable.
- [ ] A non-loopback bind is accepted only in read-only mode and refused otherwise with a clear error.
- [ ] Tests assert every write route is absent in read-only mode, the capabilities response, and the bind gating.
