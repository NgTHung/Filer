---
id: WEB-006
title: Build the v2 Tasks screen with the filter-chip menu
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002]
risk: Medium
impact: "Replaces the v1 dropdown filter bar with the v2 filter-chip menu, including its two error states, on top of the sidebar shell from WEB-005."
tags: [web, tasks]
last_updated: 2026-07-14
---

## Summary

Add the Tasks screen: a sortable table (id, title, status, priority, type, milestone, updated) over GET /api/tasks, and a '+ Filter' menu popover grouping status/priority/domain/milestone options plus free-text tag and parent inputs. Typing a tag not in the strict catalog adds an unknown_tag error pill instead of applying the filter; typing a bare local id that exists in more than one domain adds an ambiguous_reference pill with a 'did you mean' row of candidates the user can click to resolve. A 'Blocked only' toggle is a shortcut for status=Blocked. Empty state offers a clear-filters action.

## Acceptance Criteria

- [ ] The filter menu applies status, priority, domain, milestone, tag, and parent filters against GET /api/tasks without a full page reload.
- [ ] An unrecognized strict-mode tag renders as an error pill with the unknown_tag code and is excluded from the applied filter set.
- [ ] A bare local id matching more than one domain renders an ambiguous_reference pill with clickable domain-qualified candidates that resolve the filter on click.
- [ ] Column headers with a sort key toggle ascending/descending and show the active sort arrow, matching the mockup's colHeads behavior.
