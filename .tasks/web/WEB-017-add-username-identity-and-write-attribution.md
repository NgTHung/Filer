---
id: "WEB-017"
title: "Add username identity and write attribution"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015"]
risk: "Low"
impact: "Adds passwordless per-person identity so web writes can be attributed; introduces the users table and the identity cookie."
tags: ["web", "sessions", "state"]
last_updated: "2026-07-14"
---

## Summary

The team runs on a trusted LAN with no authentication, but activity history still needs to say who did what. Add passwordless identity: on first visit the UI asks for a name, the server stores it in a users table and sets a long-lived cookie, and every write handler resolves the cookie to the acting username. The sidebar footer shows the current name and lets the user change it; renaming affects future writes only. This is attribution, not access control; no endpoint checks permissions.

## Acceptance Criteria

- [ ] A first visit without an identity cookie prompts for a name and persists it in the users table with a long-lived cookie.
- [ ] Write handlers resolve the acting username from the cookie and expose it to activity recording.
- [ ] The sidebar shows the current username and can change it, affecting only future writes.
- [ ] A write without an identity cookie is rejected with a clear error asking the user to pick a name.
