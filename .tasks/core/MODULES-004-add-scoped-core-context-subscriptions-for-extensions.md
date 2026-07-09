---
id: MODULES-004
title: Add scoped core context subscriptions for extensions
status: Deferred
priority: High
type: Feature
parent: MODULES-001
milestone: "0.4.0"
depends_on: [MODULES-003]
rules: [SEMANTIC-EXTENSION-OUTPUT, SESSION-BOUNDARY]
risk: High
impact: "Lets extensions receive only the core context they declare, keeping output scoped and backpressure bounded."
tags: [extensions, events]
last_updated: 2026-07-09
---

## Summary

Add subscription contracts so an extension receives scoped core context instead of a firehose: visible nodes, current directory, selection, provider changes, and filesystem changes. Subscriptions are session-bound and respect the CORE-020 backpressure hardening.

## Rationale

Staged decomposition of MODULES-001; reactivate when MILESTONE-005 (0.4.0) work begins.

