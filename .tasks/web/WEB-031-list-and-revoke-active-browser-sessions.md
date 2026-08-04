---
id: "WEB-031"
title: "List and revoke active browser sessions"
status: In Progress
priority: "Medium"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-030"]
risk: "Low"
impact: "Gives each person visibility and control over which browsers hold their identity, and a way to cut off a browser they no longer have."
tags: ["web", "sessions", "state"]
last_updated: 2026-08-04
---

## Summary

web:WEB-030 lets one person hold many browser sessions but gives no way to see or end them, so a session on a machine you no longer use lives until the database is wiped. Add a device label to each session, recorded when the session is created, and a settings section listing your active sessions with that label and last-seen, marking the one you are using. Any other session can be revoked, and a revoked session is rejected on its next request. Revoking a session also invalidates the pairing PINs it minted, so a stolen browser cannot hand out identity after being cut off. You cannot revoke the session you are using, because that is a sign-out and not a session-management action.

## Acceptance Criteria

- [ ] Each session records a device label at creation time, derived from the request User-Agent and falling back to a stable placeholder when it is absent or unparsable.
- [ ] An authenticated user can list their active sessions with device label, created-at, and last-seen, with the session in use marked.
- [ ] A user can revoke any session other than the one in use, and the revoked session is rejected on its next request.
- [ ] Revoking a session invalidates every live pairing PIN that session minted.
- [ ] A user cannot revoke the session they are using, and the attempt is rejected with a clear error.
- [ ] The session list shows only the acting user's sessions and never another user's.
- [ ] Tests cover label capture including the missing-User-Agent fallback, revoke-then-reject, pairing PIN invalidation on revoke, and the self-revoke rejection.
