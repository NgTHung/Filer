---
id: CORE-028
title: Add large-directory benchmark harness with performance gates
status: In Progress
priority: High
type: Feature
milestone: "0.3.1"
depends_on: [PIPELINE-001]
risk: Medium
impact: "Turns 0.3.1 first-paint and paging performance criteria into measured numbers instead of vibes."
tags: [core, performance, benchmark]
last_updated: 2026-08-26
---

## Summary

Add a reproducible benchmark harness for large-directory listing and paging so milestone performance gates are measured, not asserted. Cover a System32-scale fixture (10,000+ entries) and exercise first-page latency, next-page latency, and decoration overlay cost through the public command path. Record baseline numbers in the repository so regressions are visible in review. Provisional gates below are adjustable with recorded rationale once the first baseline exists.

## Acceptance Criteria

- [ ] A benchmark harness lists a generated 10,000-entry directory through public core contracts and reports first-page and next-page latency.
- [ ] Baseline numbers are recorded in the repository with the machine profile used to produce them.
- [ ] Provisional gate: first page of a 10,000-entry local directory is delivered without waiting for the full directory walk, and the harness proves it.
- [ ] Provisional gate: decoration emission never delays listing delivery; the harness measures listing latency with decorations on and off.
- [ ] The harness runs from a single cargo command documented in the crate README.
