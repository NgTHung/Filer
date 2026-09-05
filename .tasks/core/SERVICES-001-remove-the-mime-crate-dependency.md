---
id: "SERVICES-001"
title: "Remove the mime crate dependency"
status: "To Do"
priority: "Low"
type: "Refactor"
milestone: "0.3.1"
risk: "Low"
impact: "Drops the mime crate; the detector's text-type check uses string comparison."
tags: ["mime", "dependencies", "enhancement", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

The mime crate has a single use: the mime_crate::TEXT comparison in detect_from_path. Replace it with the same top-level text-type string check categorize already relies on, then drop the dependency.

## Acceptance Criteria

- [ ] detector.rs no longer imports or references the mime crate.
- [ ] The ambiguous-extension text check uses a string comparison consistent with categorize.
- [ ] mime is removed from filer-core Cargo.toml.
- [ ] The mime_test suite passes unchanged.
