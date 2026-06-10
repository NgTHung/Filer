---
id: UTILS-001
title: Establish task-driven agent workflow
status: Done
priority: High
type: Feature
risk: Low
impact: "Changes how coding agents plan, start, verify, and complete tracked Filer work."
tags: [agent, workflow, tasks]
last_updated: 2026-06-06
---

## Summary

Provide a repository-local Codex skill that drives substantial Filer work through validated task context and lifecycle commands.

## Acceptance Criteria

- [x] The skill validates task state and inspects existing or ready work before planning substantial changes.
- [x] The skill creates or refines tasks according to project policy and applies YAGNI to scope and acceptance criteria.
- [x] The skill loads structured task context before implementation and gates lifecycle mutations on user intent and evidence.
- [x] The skill composes with relevant methodology skills when available and remains usable when they are absent.
- [x] Repository instructions require the skill for substantial task-driven planning and implementation.
- [x] Baseline and post-skill scenarios verify task creation, refinement, readiness blockers, trivial-change handling, and plan-mode behavior.
