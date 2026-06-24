---
id: PROVIDER-002
title: Stabilize provider registry and VFS contracts
status: Done
priority: Medium
type: Epic
depends_on: [VFS-001, CORE-002]
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: High
impact: "Defines the provider contracts future remote, archive, encrypted, and virtual capabilities must use."
tags: [provider, vfs, remote]
last_updated: 2026-06-24
---

## Summary

Stabilize canonical provider addressing and extension-friendly provider contracts before adding concrete remote or mount adapters.

## Exit Criteria

- [x] Provider profiles and configuration are serializable without portable secrets.
- [x] Location is canonical across local, archive, segmented archive, virtual, and extension-backed providers.
- [x] Provider connection management uses profile identifiers and capability contracts.
- [x] Archive and segmented providers remain provider-backed without requiring remote-provider stubs.
- [x] Non-local providers can ship later as extensions without app-specific integration.
- [x] S3, WebDAV, SFTP, encrypted providers, FUSE or WinFsp mount adapters, Kubernetes, sync, and cloud-placeholder behavior are deferred until their contracts fit file-manager scope.
