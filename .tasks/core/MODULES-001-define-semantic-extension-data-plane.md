---
id: MODULES-001
title: Define semantic extension data plane
status: Deferred
priority: High
type: Feature
parent: CORE-001
depends_on: [API-001, REL-001, MODULES-002]
rules: [WIRE-SAFE-EXTENSIONS, SEMANTIC-EXTENSION-OUTPUT, SESSION-BOUNDARY]
risk: High
impact: "Defines extension output consumed by desktop and future transport clients."
tags: [extensions, events, semantic-output]
last_updated: 2026-07-07
---

## Summary

Add wire-safe extension envelopes, semantic row output, and scoped core context subscriptions.

## Acceptance Criteria

- [ ] Serializable envelopes carry decorations, badges, action state, metadata, previews, and invalidations.
- [ ] Early output is limited to client-neutral file-manager semantics.
- [ ] Subscriptions can scope visible nodes, current directory, selection, provider changes, and filesystem changes.
- [ ] A trusted core host consumes validated manifests and maps permissions to sessions and provider capabilities.
- [ ] Scoped filesystem calls, contribution registration, tracing, and recoverable failures remain core-authoritative.
- [ ] The bridge depends on filer-ecosystem only when live host contracts require it.

## Rationale

Premature: filer-ecosystem has zero consumers workspace-wide and the data plane should be designed against a real consumer (MODULES-002 git decorations) rather than as a speculative wire contract. Defer until extensions are a near-term need.

MODULES-002 is now the explicit dependency: it proves the in-process semantic contract first, and this task generalizes that contract into wire-safe envelopes. This task no longer carries the 0.3.0 milestone; the 0.3.0 decoration exit criterion is satisfied by MODULES-002 alone.
