---
id: "CORE-044"
title: "Correct benchmark protocol validation regressions"
status: Done
priority: "High"
type: "Bug"
parent: "core:CORE-029"
milestone: "0.3.1"
depends_on: ["core:CORE-039"]
risk: "Medium"
impact: "Prevents invalid benchmark traces from being accepted and valid traces from being rejected before CORE-040 consumes the protocol."
tags: ["core", "benchmark", "protocol", "bug", "testing", "remediation", "ready-for-agent"]
last_updated: 2026-09-17
---

## Summary

Correct the CORE-039 review findings through the existing public parser and validator seams, then reduce allocation and module-size debt in the isolated benchmark package.

## Acceptance Criteria

- [x] Strict event parsing rejects duplicate keys at every object depth and maps non-event JSON stdout to unexpected_stdout.
- [x] Trace validation accepts either permitted page/viewport order with cumulative counts while rejecting early listing milestones and inflated continuation-proof counts.
- [x] The first-page examined-row gate is decided only from first-page evidence and preserves not_evaluable when that count is unavailable.
- [x] Canonical digesting and scenario output selection avoid unnecessary full-row clones, and shared identifier and digest syntax checks have one implementation.
- [x] Validator and trace tests are split into focused modules within repository size guidance, with public-interface regression coverage for every review finding.
- [x] The isolated package formatting, check, Clippy, all-target tests, doctests, dependency isolation checks, and task validation pass.
