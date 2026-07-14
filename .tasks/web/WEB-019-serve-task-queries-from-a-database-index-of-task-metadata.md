---
id: "WEB-019"
title: "Serve task queries from a database index of task metadata"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015"]
risk: "High"
impact: "Moves task list queries from per-request file parsing to an invalidating database index; wrong invalidation would serve stale task state, so correctness tests gate this."
tags: ["web", "tasks", "cache", "indexing", "performance"]
last_updated: "2026-07-14"
---

## Summary

Every GET /api/tasks request re-reads and re-parses the whole .tasks/ tree, which grows linearly with project size. Mirror task frontmatter into an index table and serve list, filter, and sort queries from it. The files stay the source of truth: web-driven writes update the index in the same request, and reads detect external edits (CLI or editor) by comparing file modification stamps and re-parse only stale files. A full rebuild from the files must always be possible and must produce the same results as direct parsing.

## Acceptance Criteria

- [ ] GET /api/tasks serves list, filter, and sort from the index with results identical to parsing the files directly.
- [ ] A task file changed outside the webapp is detected via its modification stamp and reindexed before results are served.
- [ ] Web-driven writes update the index within the same request so a follow-up read never sees stale data.
- [ ] A full index rebuild from the files is available and tested to match the file-derived task list.
