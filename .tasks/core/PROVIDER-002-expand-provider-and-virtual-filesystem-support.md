---
id: PROVIDER-002
title: Expand provider and virtual filesystem support
status: To Do
priority: Medium
type: Epic
depends_on: [VFS-001, CORE-002]
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: High
impact: "Adds persistent remote, archive, encrypted, and virtual provider capabilities."
tags: [provider, vfs, remote]
last_updated: 2026-06-06
---

## Summary

Complete canonical provider addressing and add extension-friendly provider implementations.

## Exit Criteria

- [ ] Provider profiles and configuration are serializable without portable secrets.
- [ ] Location is canonical across local, archive, remote, virtual, and extension providers.
- [ ] Provider connection management uses profile identifiers and capability contracts.
- [ ] Archive, SFTP, WebDAV, S3, and encrypted providers implement core-owned contracts.
- [ ] Non-local providers can ship as extensions without app-specific integration.
- [ ] FUSE and Kubernetes providers remain optional and proceed only when they fit file-manager scope.
