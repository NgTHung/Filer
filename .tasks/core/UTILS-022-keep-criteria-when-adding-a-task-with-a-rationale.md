---
id: "UTILS-022"
title: "Keep criteria when adding a task with a rationale"
status: "To Do"
priority: "Medium"
type: "Bug"
risk: "Low"
impact: "taskroot add silently drops every criterion when --rationale is passed, and the created task then fails validation."
tags: ["tooling", "bug", "ready-for-agent"]
last_updated: "2026-09-23"
---

## Summary

render_new_task in taskroot/src/lifecycle.rs returns right after writing the Rationale section, so the criteria heading and every --criterion or --checked-criterion item are never written. This affects every task type and import through NewTask. Since add reports success, the task is left on disk and fails validation with a missing criteria section. Render the criteria section before Rationale, matching the section order of existing tasks, and keep Blocked Reason placement unchanged.

## Acceptance Criteria

- [ ] A regression test adds a task with a summary, rationale, open criteria, and checked criteria, and the file contains the configured criteria heading with every item in order.
- [ ] Tasks created with a rationale place the criteria section before Rationale and pass validation without manual edits.
- [ ] Import through NewTask with a rationale keeps its criteria, covered by a test.
- [ ] Tasks created without a rationale render exactly as before.
