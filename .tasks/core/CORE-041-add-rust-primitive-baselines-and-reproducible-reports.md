---
id: "CORE-041"
title: "Add Rust primitive baselines and reproducible reports"
status: "To Do"
priority: "High"
type: "Feature"
parent: "core:CORE-031"
milestone: "0.3.1"
depends_on: ["core:CORE-040"]
risk: "Medium"
tags: ["core", "benchmark", "performance", "enhancement", "ready-for-agent"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-23"
---

## Summary

Add std::fs and Tokio flat-listing adapters matching CORE-040 requested fields. Land the adapters first, then report generation from stored raw JSON. Compare each layer separately: primitives establish an engine baseline and Filer command measurements establish a core baseline. Record observable metrics and explicit unavailable values.

## Acceptance Criteria

- [ ] Both primitive adapters validate canonical rows and metadata on the same manifests as Filer before accepting samples.
- [ ] Reports reproduce medians and supported percentiles from raw JSON, preserve cache/machine/build identity, and keep engine and core results separate.
- [ ] Tests reject requested-field mismatches and invalid samples, and a documented command produces a recorded baseline for all three adapters.
- [ ] Recorded baselines cover the existing Linux Btrfs profile and a Windows NTFS profile on the same manifests. A read-only `C:\Windows\System32` run is reported as a supplementary real-world fixture and does not gate results.

## Rationale

The Windows profile was added on 2026-09-23. Every recorded baseline so far is Linux Btrfs, and Filer targets Explorer replacement on Windows, where native Win32 file managers such as Filesmash set user expectations.
