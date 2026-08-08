---
id: "WEB-022"
title: "Publish task progression to an external read-only service"
status: "To Do"
priority: "High"
type: "Epic"
risk: "Medium"
impact: "Lets an independent service present current task progress without access to a checkout or Filer internals."
tags: ["web", "contracts", "sync", "tasks"]
last_updated: "2026-08-08"
---

## Summary

Task state only exists inside a checkout, but a separate read-only service needs normalized progress data for clients such as a mobile web app. `filer-task-web` owns the outbound boundary: validate the project, build a versioned full snapshot, and publish it to a configured HTTPS endpoint. The external project owns authentication, persistence, read APIs, deployment, and its JavaScript application. Filer does not host or implement the receiver. Contract: docs/superpowers/specs/2026-08-08-external-task-progress-publish-contract.md.

## Exit Criteria

- [ ] A versioned, language-neutral contract lets an external service consume normalized task and milestone progress without importing Rust types.
- [ ] `filer-task-web` publishes complete validated snapshots to a configured endpoint with idempotent delivery and visible delivery status.
- [ ] Raw task files, local paths, and Markdown never cross the publish boundary.
- [ ] Delivery failure never rolls back or blocks a committed local task write.
- [ ] Receiver storage, hosting, read APIs, and user interface remain outside the Filer repository.
