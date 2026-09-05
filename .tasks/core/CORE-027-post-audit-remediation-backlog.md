---
id: CORE-027
title: Post-audit remediation backlog
status: To Do
priority: Medium
type: Epic
milestone: "0.3.1"
risk: Medium
impact: "Owns CORE-004 audit remediations planned for 0.3.1 local excellence after 0.3.0 contracts."
tags: [core, audit, remediation]
last_updated: 2026-09-05
---

## Summary

Track post-audit reliability, performance, and maintainability remediations from CORE-004/CORE-013. They are not 0.3.0 exit gates. They are candidate work for milestone 0.3.1 (MILESTONE-004). Children were reparented from CORE-001 so 0.3.0 membership stays honest. Historical findings still live in docs/reviews/filer-core/VERDICT.md.

The maintainer accepted docs/adr/0001-core-runtime-lifecycle.md on 2026-09-05.
API-018/019 cover startup composition and typed compiled-in commands. REL-007
covers validation. OPS-004/005 cover bounded mutation queues and failure recovery,
followed by REL-008 for graceful completion. REL-009 separates superseded reads
from independent work. These are new reliability follow-ups, not reopened Done
tasks. Refine their needs-triage entries before implementation; milestone
membership does not add new release gates by itself.

## Exit Criteria

- [ ] CORE-017, CORE-018, CORE-019, CORE-021, CORE-022, CORE-024, and PIPELINE-003 are Done or explicitly Deferred with rationale.
- [x] No child of this epic is required to close MILESTONE-003 / 0.3.0; that milestone is Done and these children belong to 0.3.1.
- [ ] API-018, API-019, REL-007, OPS-004, OPS-005, REL-008, and REL-009 deliver the accepted runtime decisions with public-interface regression evidence.
