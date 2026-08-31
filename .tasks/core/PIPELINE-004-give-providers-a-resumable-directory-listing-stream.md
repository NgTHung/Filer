---
id: "PIPELINE-004"
title: "Give providers a resumable directory listing stream"
status: "To Do"
priority: "High"
type: "Feature"
parent: "PIPELINE-003"
milestone: "0.3.1"
rules: ["PROVIDER-ACCESS"]
risk: "Medium"
impact: "Replaces the offset rewalk in local paged listing with a retained provider handle."
tags: ["core", "vfs", "provider", "paging", "performance"]
last_updated: "2026-08-31"
---

## Summary

Stage 1 of PIPELINE-003. Add a provider contract for resuming a directory walk without replaying earlier entries. LocalFs backs it with a retained read_dir handle so a paged listing stops costing a full rewalk per page. Providers without native paging keep the existing full-listing fallback. This stage does not change paging session state or page assembly; PIPELINE-005 consumes the stream.

## Acceptance Criteria

- [ ] A provider can open a directory listing stream that yields every entry exactly once across successive batches without re-reading earlier entries.
- [ ] LocalFs paged listing no longer re-opens the directory and skips a growing prefix per page, proven by a test that counts directory opens across a multi-page walk.
- [ ] The stream honors the requested listing detail and observes cancellation between batches.
- [ ] Providers without native paging expose no stream and keep the existing full-listing fallback through an explicit default.
- [ ] Dropping a stream releases the provider handle it holds.
