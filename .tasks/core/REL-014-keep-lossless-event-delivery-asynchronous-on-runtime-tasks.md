---
id: "REL-014"
title: "Keep lossless event delivery asynchronous on runtime tasks"
status: Done
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

- [x] A regression queues more Handshake results than the event capacity on a current-thread runtime and proves both an active consumer and an unrelated timer progress; the test has an external timeout so a deadlock fails predictably.
- [x] Router and actor task paths await lossless event delivery; shared dispatch supports handlers that need asynchronous delivery without blocking an executor thread.
- [x] Small-capacity tests prove terminal and error events are retained, progress coalescing stays bounded, and shutdown/disconnection releases blocked senders.
- [x] Every changed call site is audited for blocking sends, and cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.

## Verification

`crates/filer-core/tests/lossless_delivery_test.rs` sends 1,024 Handshakes through
the public runtime with an active consumer and a separate timer. Each subprocess
has a 10-second external deadline. The test reproduced executor deadlock before
the fix. It also verifies correlated errors across 12 command paths under the
same queue pressure.

`crates/filer-core/src/tests/api/event_sink_test.rs` verifies retained errors and
completed, cancelled, and failed progress with capacity one. It checks that
shutdown and receiver disconnection release a sender waiting for capacity. The
existing coalescing tests verify bounded progress and terminal-scope storage.

The call-site audit covers lifecycle and validation handlers, router rejection,
navigation snapshots, preview cache hits, and every actor's error delivery.
Remaining synchronous actor sends target unbounded internal command channels.
The synchronous EventSink and SessionManager APIs have no runtime-task callers;
EventSink documents its blocking behavior and the asynchronous alternative.

`cargo fmt --check`, `cargo check -p filer-core`, and `cargo test -p filer-core`
pass. The core unit suite runs 840 tests; integration tests and 30 runnable
doctests also pass. Commits separate dispatch support, navigation guard handling,
actor call-site migration, and delivery verification so each stage stays below
700 changed lines.
