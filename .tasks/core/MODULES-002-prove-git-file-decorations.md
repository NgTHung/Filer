---
id: MODULES-002
title: Prove git file decorations
status: In Progress
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
depends_on: [PIPELINE-001, CORE-025, CORE-020]
rules: [SEMANTIC-EXTENSION-OUTPUT, ACTOR-LONG-WORK]
risk: Medium
impact: "Exercises asynchronous extension output on large directory listings."
tags: [extensions, git, decorations]
last_updated: 2026-08-31
---

## Summary

Use a trusted in-process prototype to prove semantic git decorations without delaying directory loading. This task defines the minimal in-process semantic decoration contract itself; the wire-safe data plane (MODULES-001) is designed later by generalizing from this proven slice, not the other way around.

Depends on CORE-025 so decoration identity is built on LocationRef rather than the NodeId compatibility surface scheduled for removal, and on CORE-020 so decoration and invalidation event streams land on the bounded-channel backpressure policy instead of the current unbounded channels.

## Acceptance Criteria

- [x] Decoration payloads use a minimal in-process semantic contract, kept small enough for MODULES-001 to later generalize into wire-safe envelopes.
- [x] Decorations address nodes by Location identity, not NodeId.
- [x] Visible nodes are the bounded input to git status work.
- [x] Modified, added, deleted, untracked, ignored, conflicted, and clean states use semantic decoration payloads.
- [x] Directory pages render before decoration work completes.
- [ ] Repository changes, including paired renames, invalidate affected decorations.
- [ ] Background Git status work disables optional index locks.
- [ ] Empty visible sets do not retain repository watches.
- [ ] A large-repository test runs the real Git backend and proves decoration work does not block directory loading.
