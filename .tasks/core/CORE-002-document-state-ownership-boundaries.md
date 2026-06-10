---
id: CORE-002
title: Document state ownership boundaries
status: To Do
priority: High
type: Docs
parent: CORE-001
milestone: "0.3.0"
rules: [CORE-LIBRARY, SESSION-BOUNDARY]
risk: Low
impact: "Prevents app configuration and portable profile state from becoming coupled."
tags: [state, profiles, sessions]
last_updated: 2026-06-06
---

## Summary

Define ownership for app configuration, core sessions, provider references, extension profiles, and future sync data.

## Acceptance Criteria

- [ ] App-local UI configuration is explicitly app-owned.
- [ ] Core session snapshots and provider profile references have defined boundaries.
- [ ] Portable profile state excludes provider secrets.
- [ ] Future sync ownership is documented without moving UI persistence into core.
