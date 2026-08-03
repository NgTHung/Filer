---
id: "WEB-029"
title: "Move project opening and creation into dialogs on Settings"
status: Done
priority: "Medium"
type: "Feature"
parent: "WEB-001"
depends_on: ["web:WEB-028"]
risk: "Low"
impact: "Removes the last place where creating a project is a checkbox on a path field, and gives both entry points one dialog shell."
tags: ["web", "tasks"]
last_updated: 2026-08-02
---

## Summary

Settings still registers projects through an inline path form with a Create new here checkbox (web:WEB-011), while the switcher now creates through a name-and-location dialog (web:WEB-028). Replace the inline form with two buttons that open dialogs: the same create dialog the switcher uses, and an open dialog holding only the path of an existing project. Extract the overlay, the outside click, and the Escape handling into a shared dialog shell so both dialogs behave identically wherever they are mounted.

## Acceptance Criteria

- [x] Settings opens an existing project through a dialog holding a single path field, with no create checkbox anywhere on the screen.
- [x] Settings creates a project through the same dialog component the switcher uses, and the new project becomes active.
- [x] Both dialogs share one shell that renders the overlay and closes on Cancel, on a click outside, and on Escape.
- [x] A refused open or create keeps its dialog open and renders the refusal inside it.
- [x] Initializing a directory that is already there is still reachable: naming a project after an existing directory under the chosen location initializes it in place.
