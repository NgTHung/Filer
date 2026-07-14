---
id: WEB-013
title: Add a dark mode theme toggle across all v2 screens
status: To Do
priority: Low
type: Feature
parent: WEB-001
depends_on: [WEB-005, WEB-006, WEB-007, WEB-008, WEB-011, WEB-012]
risk: Low
impact: "Adds a light/dark theme the v2 mockup was designed single-theme, covering every screen and the drawer and palette overlays."
tags: [web, tasks]
last_updated: 2026-07-14
---

## Summary

The v2 mockup hardcodes a light palette (#FFFFFF backgrounds, #1C1917 text) inline on every element via style.css and inline styles. Replace hardcoded colors with CSS custom properties in style.css, add a light and dark value set, default to the OS prefers-color-scheme, and add a toggle button in the sidebar footer that overrides it and persists the choice in localStorage. Status/priority dot colors and the semantic red/amber/green/blue accents keep working in both themes (adjust background/border tints, not the hue).

## Acceptance Criteria

- [ ] Every screen, the detail drawer, and the command palette render correctly in both themes with no hardcoded color left outside the CSS custom properties.
- [ ] The theme defaults to prefers-color-scheme and a sidebar toggle overrides and persists the choice across a reload via localStorage.
- [ ] Status badges, priority dots, and error/warning accents remain readable (contrast-appropriate) in both themes.
