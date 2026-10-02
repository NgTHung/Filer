---
id: "UI-017"
title: "Browse local Windows and remote Linux files in one GUI"
status: "To Do"
priority: "Medium"
type: "Feature"
depends_on: ["app:UI-015", "core:PROTOCOL-006", "core:VFS-003"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["remote", "validation", "performance", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

Windows users need local file browsing when no Linux host is connected and optional browsing of a separate Linux machine. Extend the minimal desktop validation client after local-window and remote-adapter evidence exists. Keep the renderer provisional and the first proof read-only; full framework evaluation and the application rewrite retain their separate scope.

## Acceptance Criteria

- [ ] One Windows window browses local NTFS through embedded core while Linux browsing uses the remote adapter; unavailable hosts never prevent local startup or navigation.
- [ ] Host-scoped Locations and Sessions prevent rows, selection, events, and cursors from one host being used on another; no Linux path is interpreted as a Windows path.
- [ ] Virtualized rows, recoverable connection errors, cancellation, reconnect, and paging use the proven client state model; shared behavior is extracted only from real local/remote call sites.
- [ ] Windows tests and recorded local-plus-remote smoke runs prove large-folder input remains serviceable and effects/decorations do not block browsing; applicable client/core/protocol checks pass.
