---
id: SERVICES-003
title: Migrate type detection to the file-format crate
status: To Do
priority: Medium
type: Refactor
depends_on: [SERVICES-001, SERVICES-002]
risk: High
impact: "Replaces infer and new_mime_guess internals behind MimeDetector and re-audits category routing."
tags: [mime, detection, dependencies]
last_updated: 2026-06-13
---

## Summary

Replace infer and new_mime_guess with the file-format crate behind the existing MimeDetector facade. The facade is a clean seam, but categorize() and the extension table are hand-tuned to infer and new_mime_guess MIME strings, so swapping detectors silently re-routes files unless every mapping is re-audited. Confirm file-format detects from the header byte slice without a full reader or seek (required for remote providers), and measure build-time and binary-size impact before committing.

## Acceptance Criteria

- [ ] MimeDetector produces categories through file-format without infer or new_mime_guess.
- [ ] categorize() and the extension table map file-format media types with no routing regressions across mime_test and table_test.
- [ ] Detection works from a header byte slice without requiring a full reader or seek.
- [ ] Build-time and binary-size deltas are recorded in the task before merge.
- [ ] infer and new_mime_guess are removed from filer-core Cargo.toml.
- [ ] If the weight tradeoff argues against migration, the decision and measurements are recorded and the task is closed without the swap.
