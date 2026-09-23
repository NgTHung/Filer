---
id: "CORE-033"
title: "Add external application adapters and comparative reports"
status: "Deferred"
priority: "Medium"
type: "Feature"
parent: "CORE-029"
depends_on: ["CORE-032"]
risk: "Medium"
impact: "Places Filer results beside pinned Yazi, Broot, File Explorer, and Filesmash runs without mixing application and in-process leaderboards."
tags: ["core", "performance", "benchmark", "tooling", "output", "enhancement", "needs-triage"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-23"
---

## Summary

Add isolated Yazi and Broot adapters, raw result storage, and generated peer reports. Measure terminal applications through a fixed pseudoterminal and parsed virtual screen so the benchmark observes correct visible state instead of process exit or raw escape bytes.

Measure the Windows GUI peers, File Explorer and Filesmash, through UI Automation. Their visible list state is the observable result. Filesmash is closed source, so only external timing is available.

## Acceptance Criteria

- [ ] Yazi and Broot adapters pin release, binary digest, configuration, terminal dimensions, and declared scenario capabilities.
- [ ] File Explorer and Filesmash adapters pin the Windows build or release and binary digest, and measure launch to first visible row and folder switch to visible row through UI Automation. Phases the automation tree cannot observe are `not_supported`.
- [ ] Each application runs with isolated home, configuration, cache, and state directories in a fixed pseudoterminal.
- [ ] ANSI output is parsed into a virtual terminal whose visible rows, selection, groups, and status state are checked against scenario expectations.
- [ ] Unsupported application semantics appear as `not_supported` and never as zero-duration or estimated results.
- [ ] Randomized multi-process sampling records startup, steady-state input-to-frame, CPU, peak memory, raw samples, and failure transcripts.
- [ ] Generated reports keep engine, core, reference-application, and external-application leaderboards separate and can be reproduced from stored raw JSON.

## Rationale

The maintainer approved a smaller 0.3.1 benchmark gate on 2026-09-05. External application adapters follow the initial browse journey and reports; no later release milestone is assigned.

File Explorer and Filesmash were added on 2026-09-23. Filesmash is a native C++20/Win32 Explorer alternative, which makes it the most direct Windows speed reference.
