---
id: UTILS-013
title: Add a task-editing and project-provisioning library surface
status: To Do
priority: High
type: Epic
depends_on: [UTILS-012]
risk: High
impact: "Adds the write primitives filer-task-web's v2 UI needs beyond lifecycle transitions: creating a brand-new project, mutating its policy, editing an existing task's stored fields, and toggling one checklist item."
tags: [tasks, library, portability]
last_updated: 2026-07-14
---

## Summary

core:UTILS-005 made filer-task portable across projects and core:UTILS-012 made concurrent multi-project access safe, but the library still has no write path for anything except add_task and the five lifecycle transitions. The v2 web UI (web:WEB-001) needs four more: initialize a brand-new .tasks project at an arbitrary path, mutate an open project's policy (domains, prefixes, task types, tag catalog), edit an already-created task's stored fields (title, summary, body sections, risk, impact, tags, milestone, parent, depends_on), and toggle a single acceptance/exit criteria item by index. Each is a library-level primitive other consumers (the CLI, agents) can also use, not a filer-task-web-only shortcut.

## Exit Criteria

- [ ] TaskProject can initialize a brand-new project at a path that has no .tasks/ yet, writing a valid config.json and creating no domain directories until a task exists in them.
- [ ] An open project's policy (domains, prefixes, task types, tag catalog) can be mutated in a way that re-validates against all existing task files before writing, and rejects a change that would invalidate an existing task.
- [ ] An existing task's title, summary, body sections, risk, impact, tags, milestone, parent, and depends_on can be edited through the same validation path task creation uses, and the file write is atomic.
- [ ] One acceptance/exit criteria item can be toggled by index without rewriting any other line of the task file.
- [ ] Every new library function is documented and, where a CLI equivalent makes sense, exposed as a filer-task subcommand in the same child task so CLI and library stay in parity.
