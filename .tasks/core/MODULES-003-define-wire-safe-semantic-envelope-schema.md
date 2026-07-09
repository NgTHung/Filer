---
id: MODULES-003
title: Define wire-safe semantic envelope schema
status: Deferred
priority: High
type: Feature
parent: MODULES-001
milestone: "0.4.0"
depends_on: [MODULES-002]
rules: [WIRE-SAFE-EXTENSIONS, SEMANTIC-EXTENSION-OUTPUT]
risk: High
impact: "Fixes the serializable envelope shapes every extension output and future transport client depends on."
tags: [extensions, semantic-output]
last_updated: 2026-07-09
---

## Summary

Define the serializable envelope types that carry semantic extension output: decorations, badges, action state, metadata, and invalidations. Generalize from the concrete payloads MODULES-002 emits in-process rather than designing speculatively. Envelopes are versioned, client-neutral, and contain file-manager semantics only; no widget or layout hints.

## Rationale

Staged decomposition of MODULES-001; reactivate when MILESTONE-005 (0.4.0) work begins.

