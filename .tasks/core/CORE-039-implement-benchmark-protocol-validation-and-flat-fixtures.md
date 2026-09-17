---
id: "CORE-039"
title: "Implement benchmark protocol validation and flat fixtures"
status: In Progress
priority: "High"
type: "Feature"
parent: "core:CORE-029"
milestone: "0.3.1"
depends_on: ["core:CORE-030"]
risk: "Medium"
tags: ["core", "benchmark", "performance", "enhancement", "ready-for-agent"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: 2026-09-17
---

## Summary

Implement the CORE-030 protocol and deterministic flat-10k/flat-100k manifests inside the isolated filer-core/benchmarks package. Land schema validation and golden messages first, then fixture generation as a separate commit. Fixtures expose stable relative identities and requested metadata; provider-order results use order-independent membership validation unless a scenario explicitly requests ordering. Broader fixtures belong to CORE-042.

## Execution Plan

Follow [the staged execution plan](../../docs/benchmarks/core-039-execution-plan.md). It records protocol clarifications, public test interfaces, seven implementation and verification commits, conformance coverage, fixture readback checks, and dependency-isolation evidence. Planning does not complete any acceptance criterion.

## Protocol Clarifications

- `browse.next` opens page 1 before `page-0002` through `page-0040`; page 40 carries the final 16-row page and a separate 10,000-row membership proof.
- Counts reset for each action. Repeated rows in a listing proof do not increase action counts, while sample totals cover the complete continuation chain.
- JSON type errors return `invalid_schema`; an integer protocol version other than `1` returns `unsupported_protocol_version`. Unknown phases, malformed rows, non-event JSON stdout, and plain stdout diagnostics have stable codes defined in the benchmark specification.
- Correlation, framing, sequence, row, output, and terminal checks apply to non-success traces. Missing success milestones are allowed for legitimate capability results. A success trace for an undeclared scenario returns `unsupported_reported_as_success` before milestone validation.
- The streaming unfiltered examined-row gate does not apply to filtered or snapshot-only work. CORE-042 owns sparse-filter fixtures and continuation evidence.
- Capability declarations and requested metric names are trusted validator context, not new wire fields. Setup, profile, cache, and ready-barrier evidence stays with the future runner.

## Acceptance Criteria

- [ ] Protocol tests reject version mismatches, malformed events, missing required phases, duplicate rows, wrong digests, and unsupported scenarios reported as success.
- [ ] Both flat fixtures reproduce their expected membership and requested metadata from versioned manifests; generation stays outside timed samples.
- [ ] Production and normal development dependency graphs remain unchanged, and the isolated package tests pass.
