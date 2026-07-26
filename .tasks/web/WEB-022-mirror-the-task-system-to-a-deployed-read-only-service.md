---
id: "WEB-022"
title: "Mirror the task system to a deployed read-only service"
status: "To Do"
priority: "High"
type: "Epic"
risk: "Medium"
impact: "Makes the task board reachable without a checkout by publishing .tasks/ snapshots to a remote filer-task-web that serves them read-only, while files stay the only source of truth."
tags: ["web", "server", "sync", "tasks"]
last_updated: "2026-07-26"
---

## Summary

Task state only exists inside a git checkout, so anyone without one cannot see the board. This epic adds a deployed mirror: a second filer-task-web instance on a remote host that receives published snapshots of .tasks/ and serves the same screens with no write path. Snapshots carry raw task files rather than parsed records, so the mirror materializes them into a directory it registers as an ordinary project root and every existing read route, validation, and screen works unchanged. The local daemon publishes because it already holds the project registry and validates on every write, and the snapshot builder lives in the filer-task library so a git hook or CI can publish without it. The mirror is derived state: deleting its data directory loses nothing a republish cannot restore, and no task content ever flows back into a repository. Design: docs/superpowers/specs/2026-07-26-task-mirror-sync-design.md.

## Exit Criteria

- [ ] A remote filer-task-web serves the Filer task board from published snapshots with no checkout on its host.
- [ ] Several projects publish into one mirror, each with its own credential and freshness setting.
- [ ] The mirror exposes no write route and its frontend hides every write affordance.
- [ ] A repository with validation errors cannot overwrite a good mirror snapshot.
- [ ] Task content flows only from checkout to mirror; deleting the mirror data directory loses no task data.
