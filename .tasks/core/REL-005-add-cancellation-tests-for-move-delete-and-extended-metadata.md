---
id: REL-005
title: Add cancellation tests and review metadata-load cancellation
status: To Do
priority: Medium
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Medium
impact: "Closes cancellation test gaps and proves whether metadata-load can stay non-cancellable without becoming a DOS surface."
tags: [reliability, testing, cancellation]
last_updated: 2026-06-21
---

## Summary

Move, Delete, and Extended-Metadata loading already arm cancellation but lack tests. Add mid-flight cancellation tests reusing the MockProvider/MockPreviewProvider delay_ms + yield_now patterns.

The prior REL-005 wording treated metadata-load as non-cancellable by design. The only backing found is the CORE-008 async actor review: `docs/reviews/filer-core/async-actors.md` says previewer metadata dispatch is never armed, is stale-guarded, and was rated low severity because it was assumed to be a single bounded metadata read. That review also says to document that choice or arm the path for consistency. It does not prove the behavior is safe. Current code still calls `provider.metadata(&path, &ProviderCx::none())` in `dispatch_metadata` and `dispatch_metadata_location`, so cancellation and deadlines cannot stop a slow provider metadata call.

Review whether that assumption still holds. Treat the current non-cancellable behavior as unproven until the task records the reason, the boundedness assumptions, and the DOS risk. If the behavior is not defensible, bring metadata-load cancellation into this task or create a follow-up bug before marking the criteria complete. Out of scope: rename, create-file, and create-folder. Provider-call timeout is owned by PROVIDER-001.

## Acceptance Criteria

- [ ] A test cancels a Move mid-flight and asserts it stops without a success event.
- [ ] A test cancels a Delete mid-flight and asserts it stops without a success event.
- [ ] A test cancels an Extended-Metadata load mid-flight and asserts no stale metadata event is emitted.
- [ ] LoadMetadata and LoadMetadataLocation cancellation behavior is reviewed against DOS risk, with the reason for keeping or changing the current ProviderCx::none path documented in the task or implementation notes.
