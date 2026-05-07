# filer-ecosystem Roadmap

This roadmap tracks the ecosystem layer: extension contracts, packages, profile
sync, and app/core/web integration points. It carries the same product message
as the app roadmap: Filer should be a fast Explorer replacement with useful
programmer support, not a bloated IDE.

The ecosystem has two contracts:

- The declaration plane: manifests, registries, permissions, packages, and
  profile operations.
- The data plane: structured semantic output emitted by extensions through core
  for clients to render, such as file decorations, badges, metadata updates,
  preview payloads, panel data, context action state, diagnostics, and
  invalidation events.

Extensions should improve file-manager mechanics without owning desktop or web
widgets directly.

The first implementation target is one complete git file decoration vertical
slice. Broader host/runtime work should wait until that slice proves context
subscription, invalidation, semantic output, permission boundaries, and client
rendering.

The first host may be trusted and in-process. It should not be described as a
general third-party plugin runtime until sandboxing and runtime permission
enforcement exist.

Milestone labels:

- `Contract`: shared data model and validation.
- `Host`: core/app integration and runtime hosting.
- `Sync`: profile operations, pack/unpack, and transport.
- `Trust`: permissions, signing, sandboxing, and security.
- `Future`: marketplace and broader ecosystem work.

## Current Baseline

- [x] `Contract` Runtime-free `filer-ecosystem` crate.
- [x] `Contract` Extension manifest model.
- [x] `Contract` WASM and native-trusted runtime tags.
- [x] `Contract` Permission model for filesystem, network, process, secrets,
  UI, providers, and sync.
- [x] `Contract` Command, event, UI, preview, metadata, converter, theme,
  icon-pack, provider, and sync contribution types.
- [x] `Contract` Registry validation for schema version, identifiers, duplicate
  command keys, and undeclared permissions.
- [x] `Contract` `.filerpack` package metadata model.
- [x] `Contract` Package validation for unsafe paths, duplicate paths, and
  invalid checksums.
- [x] `Sync` Local profile operation model.
- [x] `Sync` Idempotent profile operation application.
- [ ] `Contract` Extension output envelope model for semantic data emitted by
  running extensions.
- [ ] `Contract` File decoration model for git-style row state, badges, theme
  tokens, tooltips, and invalidation.

## First Vertical Slice: Git Decorations

- [ ] `Contract` Define file decoration payloads for modified, added, deleted,
  untracked, ignored, conflicted, and clean states.
- [ ] `Contract` Define decoration identity using `NodeId`, provider path, or a
  future provider-aware file identity.
- [ ] `Contract` Define invalidation events for repository state and visible
  directory changes.
- [ ] `Host` Let a trusted prototype extension receive visible-node context
  from core.
- [ ] `Host` Emit `FileDecorationsUpdated`-style semantic output through core.
- [ ] `Host` Ensure slow decoration work cannot block directory loading.
- [ ] `Host` Add cancellation, timeout, and stale-output behavior for the
  decoration flow.
- [ ] `Host` Define decoration cache invalidation rules; extension caches are
  allowed only when core can invalidate or age out stale output.
- [ ] `App` Render decoration badges/theme tokens from core events without
  extension-owned widgets.
- [ ] `Validation` Prove the same payload could be rendered by a future web
  client without changing the extension.

## Manifest And Registry

- [x] `Contract` Parse and serialize extension manifests.
- [x] `Contract` Reject unsupported schema versions.
- [x] `Contract` Reject duplicate extension ids.
- [x] `Contract` Reject duplicate command keys.
- [x] `Contract` Reject contribution permissions not declared by the manifest.
- [ ] `Contract` Add stable examples for git, theme, preview, converter, and
  provider manifests.
- [ ] `Contract` Add manifest docs for every field and contribution type.
- [ ] `Contract` Add registry query APIs for command palette, context menu,
  panels, previewers, themes, icon packs, and providers.
- [ ] `Contract` Add registry query APIs for output-capable extensions, such as
  file decorators, status badge producers, metadata producers, and panel data
  producers.
- [ ] `Contract` Add compatibility metadata for minimum app/core versions.
- [ ] `Contract` Add deprecation strategy for future schema versions.

## Core And Host Integration

- [ ] `Host` Add a wire-safe extension command/event envelope.
- [ ] `Host` Add a wire-safe extension output envelope for file decorations,
  status badges, panels, metadata updates, preview payloads, action state, and
  diagnostics.
- [ ] `Host` Keep `Command::Extension` available for trusted in-process Rust,
  but route public extension traffic through serializable envelopes.
- [ ] `Host` Add a core extension host module that registers validated command
  keys.
- [ ] `Host` Map extension permissions to session policies and provider
  capabilities.
- [ ] `Host` Add scoped filesystem host calls for extensions.
- [ ] `Host` Let extensions subscribe to scoped core context such as current
  directory, visible nodes, selected nodes, provider changes, and filesystem
  changes.
- [ ] `Host` Add cancellation, timeout, and progress reporting for extension
  commands.
- [ ] `Host` Add extension logging that flows through the existing tracing
  setup.
- [ ] `Host` Add recoverable extension errors as app-visible events.

## App Integration

- [ ] `Host` Extension manager UI for installed, enabled, disabled, and failed
  extensions.
- [ ] `Host` Permission review UI before enabling an extension.
- [ ] `Host` Command palette entries from enabled extension commands.
- [ ] `Host` Context menu entries from enabled extension commands.
- [ ] `Host` Sidebar, panel, status badge, and file row decoration surfaces.
- [ ] `Host` Render file decorations from semantic output rather than
  extension-owned UI widgets.
- [ ] `Host` Render extension panel/status data from client-neutral payloads.
- [ ] `Future` Arbitrary extension tabs, popups, panels, and layout control
  after row decorations and actions are proven.
- [ ] `Host` Theme and icon-pack selection from ecosystem contributions.
- [ ] `Host` Provider connection UI from provider contributions.
- [ ] `Host` Clear failure states when an extension cannot load or execute.

## Runtime Execution

- [ ] `Future` WASM runtime host with a small, explicit host API after the git
  decoration slice validates the contracts.
- [ ] `Future` WASM filesystem calls scoped by session policy.
- [ ] `Future` WASM settings and profile-state access.
- [ ] `Future` WASM command/event bridge.
- [ ] `Future` WASM output bridge for decorations, badges, metadata, previews,
  panels, action state, and invalidations.
- [ ] `Future` WASM logging bridge.
- [ ] `Trust` WASM resource limits for time, memory, and output size.
- [ ] `Host` Native trusted module bridge for built-ins and explicit power
  integrations.
- [ ] `Trust` Native trusted modules clearly marked as trusted software, not
  normal sandboxed marketplace extensions.

## Pack And Unpack

- [x] `Contract` Package file metadata and checksum fields.
- [x] `Contract` Signature metadata fields reserved in the package model.
- [ ] `Sync` Deterministic `.filerpack` pack command.
- [ ] `Sync` Safe unpack command that verifies metadata before install.
- [ ] `Sync` Install packages disabled by default.
- [ ] `Sync` Store installed packages in the local profile.
- [ ] `Sync` Exclude secrets from portable packages by default.
- [ ] `Trust` Verify package checksums before install.
- [ ] `Trust` Add optional package signature verification.
- [ ] `Future` Add package export/import UI in `filer-app`.

## Profile Sync

- [x] `Sync` Operation types for install, remove, enable, disable, config,
  theme, provider profile, and workspace updates.
- [x] `Sync` Idempotent operation application by operation id.
- [ ] `Sync` Persist ecosystem profile state and operation log for extensions,
  packages, provider profiles, workspaces, and future sync.
- [ ] `Sync` Keep simple app-local settings out of the operation log until a
  real sync/export requirement exists.
- [ ] `Sync` Add profile schema migration tests.
- [ ] `Sync` Add last-writer-wins behavior for simple settings.
- [ ] `Sync` Preserve unknown future operations for forward compatibility.
- [ ] `Sync` Add encrypted export/import for profiles that include secrets.
- [ ] `Future` Stream profile operations through `filer-server`.
- [ ] `Future` Sync desktop and web clients through the same operation log.

## Security And Trust

- [x] `Trust` Declarative permissions in manifests.
- [x] `Trust` Registry rejects undeclared permission use.
- [ ] `Trust` Permission review before enabling extensions.
- [ ] `Trust` Disable-by-default installs for third-party packages.
- [ ] `Trust` Runtime enforcement for filesystem, process, network, and secret
  permissions.
- [ ] `Trust` Scoped provider access for extension commands.
- [ ] `Trust` OS keychain or encrypted local profile storage for secrets.
- [ ] `Trust` Clear distinction between sandboxed WASM and native trusted
  modules.
- [ ] `Future` Marketplace trust model with signing keys and revocation.

## Programmer-Oriented Extensions

- [ ] `Host` Git status badge extension example.
- [ ] `Host` Git file decoration example that emits modified, added, deleted,
  untracked, ignored, conflicted, and clean states for visible files.
- [ ] `Host` Git command palette and panel extension example.
- [ ] `Host` External terminal/open-in-editor command example.
- [ ] `Host` Converter extension example.
- [ ] `Host` Syntax-aware metadata extension example.
- [ ] `Host` Theme and icon-pack extension examples.
- [ ] `Future` SSH/SFTP provider extension example.
- [ ] `Future` Project task/script launcher extension example.

## Validation Checklist

Run these before calling an ecosystem milestone complete:

```bash
cargo fmt
cargo check -p filer-ecosystem
cargo test -p filer-ecosystem
cargo check --workspace
```

Manual checks:

- [ ] Manifest examples explain what will appear in app/core/web.
- [ ] Output examples show how an extension emits semantic data and how clients
  may visualize it differently.
- [ ] `[x]` roadmap items correspond to implemented contract-layer behavior.
- [ ] Docs do not claim WASM/native runtime execution exists yet.
- [ ] Docs keep Filer positioned as a fast file manager with programmer support,
  not a full IDE.
