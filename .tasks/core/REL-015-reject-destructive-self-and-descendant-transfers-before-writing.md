---
id: "REL-015"
title: "Reject destructive self and descendant transfers before writing"
status: In Progress
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["PROVIDER-ACCESS", "SESSION-BOUNDARY"]
risk: "High"
tags: ["operations", "validation", "bug", "ready-for-agent"]
last_updated: 2026-10-02
---

## Summary

Copying a file into its own parent can truncate the source, and copying a directory into its descendant can recurse into the destination until the process aborts. Add real-provider regressions before extracting shared transfer preflight. Check filesystem identity and ancestry, including supported aliases, before either copy or move changes its destination. Path text alone cannot establish object identity.

## Acceptance Criteria

- [ ] Public Copy rejects a file copied into its own parent and onto a hard-link or symlink alias, preserving source and destination bytes.
- [ ] Copy and Move reject identical directory targets and descendant destinations, including destinations reached through symlinked ancestors; missing destination suffixes are checked through existing ancestors.
- [ ] Rejection reports all supplied Session, Request, and Operation identities and performs no destination creation, overwrite, source removal, or recursive traversal.
- [ ] Shared preflight owns the checks, provider limitations fail explicitly, valid transfers still work, and dangerous unfixed reproducers run only in isolated subprocesses.
- [ ] Real-provider tests cover supported platforms and aliases without sleep-based ordering, and cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
