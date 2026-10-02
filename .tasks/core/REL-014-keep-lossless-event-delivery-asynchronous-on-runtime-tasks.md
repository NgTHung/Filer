---
id: "REL-014"
title: "Keep lossless event delivery asynchronous on runtime tasks"
status: In Progress
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["ACTOR-LONG-WORK", "SESSION-BOUNDARY"]
risk: "High"
tags: ["async", "events", "bug", "ready-for-agent"]
last_updated: 2026-10-02
---

## Summary

A full lossless event queue can block the executor thread that must drain it. EventSink::send uses blocking sends from runtime task paths. First reproduce this with an active consumer and a single-thread Tokio executor, then make runtime delivery await capacity while keeping terminal events lossless and retained memory bounded. This fixes executor progress; independent Session delivery remains REL-011.

## Acceptance Criteria

- [ ] A regression queues more Handshake results than the event capacity on a current-thread runtime and proves both an active consumer and an unrelated timer progress; the test has an external timeout so a deadlock fails predictably.
- [ ] Router and actor task paths await lossless event delivery; shared dispatch supports handlers that need asynchronous delivery without blocking an executor thread.
- [ ] Small-capacity tests prove terminal and error events are retained, progress coalescing stays bounded, and shutdown/disconnection releases blocked senders.
- [ ] Every changed call site is audited for blocking sends, and cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
