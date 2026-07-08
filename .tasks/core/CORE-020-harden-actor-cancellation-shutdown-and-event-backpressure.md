---
id: CORE-020
title: Harden actor cancellation, shutdown, and event backpressure
status: To Do
priority: High
type: Bug
parent: CORE-001
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Medium
impact: "Shutdown can return while work continues, a fallback scan ignores cancel, and unbounded event streams can grow without limit. Gates MODULES-002: decoration event streams must land on the backpressure policy, not on unbounded channels."
tags: [core, audit, remediation, async]
last_updated: 2026-07-07
---

## Summary

Three actor-lifecycle gaps weaken the non-blocking large-directory target. First, FilerCore::shutdown aborts the tracked actor loops and calls cancel_all, which only sets the atomic flag; detached per-command tasks are not joined, so a copy or scan can keep touching the filesystem after shutdown returns. Track per-command handles in a per-actor JoinSet and await them after cancel_all, or document shutdown as fire-and-forget. Second, the fallback paging path runs PageSelection::extend over the whole directory with no is_cancelled check, so a cancel is not observed until the loop finishes; thread the token into extend and check it every N entries, matching the native page cadence. Third, every channel is unbounded; adopt the VERDICT backpressure policy: bounded channels for high-volume event streams, coalescing progress events on overflow and blocking on page and change events, with command channels left unbounded.

## Acceptance Criteria

- [ ] A shutdown-while-busy test asserts no filesystem activity continues after FilerCore::shutdown returns, or shutdown is explicitly documented as fire-and-forget with the runtime drop as the real stop.
- [ ] PageSelection::extend checks the cancellation token periodically so a fallback-provider scan of a very large directory stops promptly, pinned by a test.
- [ ] High-volume event streams use bounded channels with the documented coalesce-or-block overflow policy; command channels stay unbounded.
