# filer-ecosystem

`filer-ecosystem` defines the shared data contracts for Filer extensions,
packages, and profile synchronization.

The crate is runtime-free by design. It does not execute WASM, load native
plugins, render UI, or perform file operations. Instead, it provides the
wire-safe types that `filer-core`, `filer-app`, future web clients, and package
tools can agree on.

The intended extension model has two parts:

- A declaration plane: manifests describe commands, providers, previews,
  metadata, UI surfaces, permissions, packages, and profile participation.
- A data plane: running extensions publish structured semantic output through
  core, such as file decorations, status badges, action state, metadata updates,
  preview payloads, panel data, and invalidation events.

Clients render that semantic output in their own UI. For example, a git
extension reports that a file is modified or added; the desktop app or web
client chooses how to display that state.

The first runtime-facing slice should be git file decorations, not a complete
marketplace or broad plugin host. That slice proves the model end to end:
visible file context from core, semantic decoration output from an extension,
and client-owned rendering.

The first host can be trusted and in-process. It should be described as a
trusted core add-on model until sandboxing, package installation, marketplace
trust, and permission enforcement actually exist.

## Current Scope

- Extension manifests with runtime, permissions, commands, UI contributions,
  preview/metadata providers, converters, themes, icon packs, providers, and
  sync participation.
- Planned extension output contracts for semantic file decorations, status
  badges, action state, panel data, metadata updates, and preview payloads.
- Registry validation for schema version, identifiers, duplicate command keys,
  and undeclared permissions.
- `.filerpack` package metadata validation.
- Local profile operation types for future sync and pack/unpack workflows.

Local app settings such as bookmarks, recent paths, theme, layout, and sort
preferences should stay in simple app-owned config for now. `filer-ecosystem`
profile operations are reserved for extension/package state, provider profiles,
workspace state, and future sync.

## Runtime Direction

The intended runtime model is hybrid:

- WASM for portable, sandboxed third-party extensions.
- Native trusted modules for built-ins and explicitly trusted integrations.

Execution, sandboxing, and UI integration will be added in higher-level crates
after this contract layer stabilizes.

Extensions should not directly render app widgets. They should produce
client-neutral data that `filer-core` can route and each client can visualize.

The runtime should grow only after the contract layer proves one vertical slice.
WASM hosting, package installation, marketplace behavior, and extension manager
UI should not block the git decoration contract.

Early UI output should stay narrow: file row decorations, status badges,
context/command actions, preview payloads, and metadata payloads. Arbitrary
tabs, popups, panels, and layout control should wait.

## More Detail

- [DESIGN.md](DESIGN.md) explains the architecture choices and tradeoffs.
- [ROADMAP.md](ROADMAP.md) tracks the ecosystem checklist and next milestones.
