---
id: SERVICES-003
title: Evaluate type detection with the file-format crate
status: To Do
priority: Low
type: Design
milestone: "0.3.1"
depends_on: [SERVICES-002]
risk: High
impact: "Measures whether replacing the existing detectors improves cost without changing category routing or provider reads."
tags: [mime, detection, dependencies]
last_updated: 2026-09-05
---

## Summary

Compare file-format with the existing MimeDetector implementation in an isolated experiment. Use mime_test and table_test cases to evaluate category routing and provider-compatible header reads. Record build-time, binary-size, and detection-cost deltas under the same toolchain and features. The deliverable is a measured retain-or-migrate decision. A favorable result creates a separate implementation task before production dependencies change; an unfavorable result closes this evaluation without claiming a migration occurred. SERVICES-001 cleanup is independent of this decision.

## Acceptance Criteria

- [ ] A reproducible comparison records category agreement and every mismatch against existing MIME and extension-table fixtures.
- [ ] The experiment proves header-byte-slice support or records the missing capability, including short and empty input behavior.
- [ ] Build-time, binary-size, and detection-cost measurements record baseline/candidate versions, features, toolchain, and commands.
- [ ] A retain-or-migrate decision follows the evidence. A migration decision links a separate implementation task with routing, dependency-removal, and regression criteria; a retain decision records its rationale.
