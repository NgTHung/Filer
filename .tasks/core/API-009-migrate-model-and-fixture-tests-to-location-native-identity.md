---
id: API-009
title: Migrate model and fixture tests to Location-native identity
status: Done
priority: High
type: TestDebt
parent: API-005
milestone: "0.3.0"
rules: [CORE-LIBRARY]
risk: Low
impact: "Ports model-layer and fixture tests off NodeId so the identity model can change without deleting coverage."
tags: [api, nodeid, location, testing]
last_updated: 2026-07-09
---

## Summary

Port the model-layer and shared-fixture tests that assert on NodeId or FileNode identity to LocationRef or NodeEntry identity. As of 2026-07-09 this cluster is model_test.rs (14 sites), location_test.rs (3), query_test.rs (3), pipeline_test/fixtures.rs (4), and dir_cache_test.rs (2) under filer-core/src/tests. Fixtures come first because the module and router clusters build on them.

## Acceptance Criteria

- [x] Tests in this cluster assert on LocationRef or NodeEntry identity instead of NodeId or FileNode identity, except the explicit NodeId determinism compatibility pin that remains until NodeId is removed.
- [x] Shared fixtures expose Location-native constructors so dependent test clusters need no NodeId plumbing.
- [x] The full filer-core suite passes with no reduction in test count.
