---
id: CORE-023
title: Repair filer-core README prose and stale rustdoc
status: To Do
priority: Medium
type: Docs
parent: CORE-004
risk: Low
impact: "The READMEs are the first thing a contributor reads and currently read as machine noise."
tags: [core, audit, remediation, docs]
last_updated: 2026-06-13
---

## Summary

A command-rename find-and-replace pass corrupted README prose, replacing ordinary verbs with internal type names: sentences that should read scan, search, watch now read ScanPathCompat, SearchNodeCompat, WatchNodeCompat. Root README.md has five such hits and filer-core/README.md has several in the flow-describing prose; the Command API and migration tables that legitimately name command variants must be preserved, so a blind reverse-replace would re-break them. The filer-core/README.md Modules table is also structurally stale: it lists a bus/ directory that does not exist, attributes the workers to actors/ when they live under modules/, and omits modules/ entirely. Separately, vfs/local.rs:214 carries a # TODO rustdoc section listing implementation steps that the method below already implements, so it renders as a stub in published docs.

## Acceptance Criteria

- [ ] README.md and filer-core/README.md prose use plain verbs; *Compat names remain only where a sentence genuinely names a command variant, and the command tables are untouched.
- [ ] The filer-core/README.md Modules table drops bus/, adds modules/, and places the workers under modules/ with actors/ as infrastructure.
- [ ] The stale # TODO rustdoc section on vfs/local.rs:214 is removed or replaced with a one-line WHY note.
