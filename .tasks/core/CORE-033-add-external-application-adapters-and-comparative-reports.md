---
id: "CORE-033"
title: "Add external application adapters and comparative reports"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "CORE-029"
milestone: "0.3.1"
depends_on: ["CORE-032", "CORE-034"]
risk: "Medium"
impact: "Places Filer results beside pinned Yazi and Broot runs without mixing application and in-process leaderboards."
tags: ["core", "performance", "benchmark", "tooling", "output"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-08-26"
---

## Summary

Add isolated Yazi and Broot adapters, raw result storage, and generated peer reports. Measure terminal applications through a fixed pseudoterminal and parsed virtual screen so the benchmark observes correct visible state instead of process exit or raw escape bytes.

## Acceptance Criteria

- [ ] Yazi and Broot adapters pin release, binary digest, configuration, terminal dimensions, and declared scenario capabilities.
- [ ] Each application runs with isolated home, configuration, cache, and state directories in a fixed pseudoterminal.
- [ ] ANSI output is parsed into a virtual terminal whose visible rows, selection, groups, and status state are checked against scenario expectations.
- [ ] Unsupported application semantics appear as `not_supported` and never as zero-duration or estimated results.
- [ ] Randomized multi-process sampling records startup, steady-state input-to-frame, CPU, peak memory, raw samples, and failure transcripts.
- [ ] Generated reports keep engine, core, reference-application, and external-application leaderboards separate and can be reproduced from stored raw JSON.
