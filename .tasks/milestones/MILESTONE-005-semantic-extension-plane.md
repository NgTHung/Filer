---
id: MILESTONE-005
title: Semantic extension plane
status: To Do
priority: High
type: Milestone
milestone: "0.4.0"
depends_on: [MILESTONE-004]
risk: High
impact: "Turns the in-process git decoration prototype into a portable semantic extension data plane."
tags: [extensions, semantic-output, draft]
last_updated: 2026-09-05
---

## Summary

Wire-safe semantic extension output generalized from the MODULES-002 git decoration prototype. Clients render semantic payloads; extensions do not own desktop widgets. This is not a marketplace or WASM host milestone.

MODULES-001 is decomposed into staged children so this milestone is a sequence of reviewable tasks, not one monolith: MODULES-003 (envelope schema) then MODULES-004 (scoped subscriptions) and MODULES-005 (trusted host) then MODULES-006 (git decoration vertical slice as the exit proof). All stay Deferred until this milestone starts; reactivate the epic and children together.

## Draft policy

This milestone is a draft plan. You or any agent may modify it as much as needed (exit criteria, membership, priority, depends_on, title, or replacement by a better split) until work for 0.4.0 has started. Work has started when this milestone or any task with `milestone: "0.4.0"` first moves to `In Progress`. Until then, treat this file as editable intent, not a locked commitment. After work starts, change scope only deliberately and record why.

## Candidate membership

- MODULES-001 epic and children MODULES-003, MODULES-004, MODULES-005, MODULES-006 (all Deferred; reactivate when 0.4.0 work begins)
- CORE-003 (owned solely by this milestone; 0.3.1 no longer references its light path)
- PREVIEW-001: PREVIEW-002 specifies contracts, PREVIEW-003 stages provider-safe implementation, and PREVIEW-004 integrates the completed host contracts
- Ecosystem output-contract tasks when filed under ecosystem domain

## Exit Criteria

- [ ] MODULES-003 is Done: versioned serializable envelopes cover decorations, badges, action state, metadata, and invalidations, pinned by round-trip tests.
- [ ] MODULES-004 is Done: session-bound scoped subscriptions deliver core context without letting a slow subscriber block directory loading.
- [ ] MODULES-005 is Done: the trusted in-process host validates manifests, maps permissions to sessions and provider capabilities, and disables failing extensions without poisoning listing.
- [ ] MODULES-006 is Done: git decorations flow through the envelope data plane, listing latency stays within recorded tolerance of the CORE-028 baseline, and the direct in-process contract is deleted.
- [ ] filer-ecosystem output contracts align with core envelopes for the git decoration vertical slice.
- [ ] PREVIEW-003 provider-safe core criteria are Done; PREVIEW-004 extension integration may remain open with rationale recorded here if host pieces are staged.
- [ ] CORE-003 terminal and editor helpers launch through client-neutral commands and stay optional file-manager actions, not IDE features.
