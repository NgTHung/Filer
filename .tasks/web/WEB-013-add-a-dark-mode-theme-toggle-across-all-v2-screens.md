---
id: WEB-013
title: Add a light/dark theme toggle across all screens
status: To Do
priority: Low
type: Feature
parent: WEB-001
depends_on: [WEB-005, WEB-006, WEB-007, WEB-008, WEB-011, WEB-012]
risk: Low
impact: "Adds a light theme and a toggle to the currently dark-only stylesheet, covering every screen and the drawer and palette overlays."
tags: [web, tasks]
last_updated: 2026-07-15
---

## Summary

static/style.css already defines its palette as CSS custom properties in :root (--bg, --panel, --border, --text, --muted, --accent), but only with dark values and with a few hardcoded colors left in the error toast. Add a light value set alongside the dark one, migrate the remaining hardcoded colors (the error box reds and any added by the screen tasks) into the custom properties, default to the OS prefers-color-scheme, and add a toggle button in the sidebar footer that overrides it and persists the choice in localStorage. Status/priority dot colors and the semantic red/amber/green/blue accents keep working in both themes (adjust background/border tints, not the hue).

## Acceptance Criteria

- [ ] Every screen, the detail drawer, and the command palette render correctly in both themes with no hardcoded color left outside the CSS custom properties, including the error toast colors.
- [ ] The theme defaults to prefers-color-scheme and a sidebar toggle overrides and persists the choice across a reload via localStorage.
- [ ] Status badges, priority dots, and error/warning accents remain readable (contrast-appropriate) in both themes.
