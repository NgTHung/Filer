---
id: MODULES-002
title: Prove git file decorations
status: To Do
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
depends_on: [MODULES-001, PIPELINE-001]
rules: [SEMANTIC-EXTENSION-OUTPUT, ACTOR-LONG-WORK]
risk: Medium
impact: "Exercises asynchronous extension output on large directory listings."
tags: [extensions, git, decorations]
last_updated: 2026-06-06
---

## Summary

Use a trusted in-process prototype to prove semantic git decorations without delaying directory loading.

## Acceptance Criteria

- [ ] Visible nodes are the bounded input to git status work.
- [ ] Modified, added, deleted, untracked, ignored, conflicted, and clean states use semantic decoration payloads.
- [ ] Directory pages render before decoration work completes.
- [ ] Repository changes invalidate affected decorations.
- [ ] A large-repository test proves decoration work does not block directory loading.
