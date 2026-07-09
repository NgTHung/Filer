---
id: CORE-003
title: Define programmer helper contracts
status: To Do
priority: Low
type: Epic
milestone: "0.4.0"
depends_on: [MODULES-001]
rules: [CORE-LIBRARY, SEMANTIC-EXTENSION-OUTPUT]
risk: Medium
impact: "Adds developer-oriented actions without turning core into an IDE."
tags: [programmer, actions, extensions]
last_updated: 2026-07-09
---

## Summary

Expose lightweight project and external-tool helpers through extension-friendly contracts.

## Exit Criteria

- [ ] Terminal and editor actions launch external tools through client-neutral commands.
- [ ] Repository detection supports browsing and decoration invalidation.
- [ ] Converters and task launchers remain external actions rather than build systems.
- [ ] Git helper commands remain optional file-manager actions.
- [ ] An integrated terminal contract proceeds only without IDE diagnostics or execution ownership.
- [ ] Optional git, converter, syntax, terminal, and provider extras prefer extension implementations.
