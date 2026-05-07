# filer-ecosystem Design

`filer-ecosystem` is the shared contract layer for Filer extensions, extension
packages, and profile synchronization. It exists so `filer-core`, `filer-app`,
future web clients, and packaging tools can agree on the same vocabulary without
coupling execution runtimes or UI frameworks together.

## Design Goals

Filer should stay a fast Explorer replacement with programmer-oriented
extensions. The ecosystem must support git, terminals, SSH/SFTP, previews,
converters, themes, providers, and automation, but those features should extend
the file manager instead of turning it into a full IDE.

The ecosystem design optimizes for:

- Stable contracts before runtime execution.
- Portable extension metadata across desktop, core, and future web clients.
- Explicit permissions and contribution surfaces.
- Semantic extension output that core can route and every client can render.
- One complete vertical slice before broad platform expansion.
- Safe third-party extension paths without blocking trusted built-ins.
- Syncable local profile state that can later move through a server protocol.

## Why This Is A Separate Crate

The ecosystem layer is not part of `filer-core` because core must stay focused
on sessions, actors, providers, commands, events, previews, operations, and
search. Extension packaging and profile synchronization are product/platform
concerns that multiple frontends need to understand.

Keeping these types in `filer-ecosystem` gives the project a narrow shared
boundary:

- `filer-core` can consume validated extension commands and capabilities.
- `filer-app` can render commands, panels, themes, icon packs, and provider UI.
- Future web/server crates can serialize the same contracts.
- Package tools can validate `.filerpack` metadata without linking the app or
  core runtime.

This crate deliberately does not execute WASM, load native plugins, render UI,
or perform file operations.

The ecosystem should not force core mechanics to become third-party plugins.
Navigation, scan orchestration, search orchestration, watch, file operations,
sessions, provider routing, cache, pipeline, and event delivery stay in core.
The ecosystem extends those mechanics with optional capabilities and semantic
outputs.

## Runtime-Free Contract First

The first ecosystem milestone is manifest and registry validation, not plugin
execution. This is intentional.

Running extensions before the manifest model is stable would force security,
sync, packaging, and UI decisions into ad hoc runtime code. A runtime-free
contract layer lets the project validate:

- Which commands an extension owns.
- Which UI surfaces it contributes to.
- Which permissions it needs.
- Which providers, previewers, metadata readers, converters, themes, or icon
  packs it exposes.
- Whether package metadata is safe to unpack.

Execution can then be added behind these validated contracts.

This crate should also define the shape of extension-produced data before a full
runtime exists. The manifest says what an extension can contribute; the output
contracts say what the extension can publish after core invokes or observes it.
Examples include file decorations, badges, status kinds, context action state,
metadata updates, preview payloads, panel data, diagnostics, and invalidation
events.

The first output contract should be the git file decoration slice. It is small
enough to implement, useful enough to validate the product direction, and rich
enough to exercise context subscription, invalidation, output routing, and
client rendering.

## Hybrid Extension Model

The planned runtime model is hybrid:

- WASM for portable, sandboxed third-party extensions.
- Native trusted modules for built-ins and explicit power integrations.

The first runtime-facing implementation may be trusted and in-process. That is a
trusted core add-on model, not a marketplace-safe plugin system. It is suitable
for proving the git decoration data plane without paying the cost of sandboxing
too early.

WASM remains the likely public extension target later because it is portable
across app, core, and future web/server contexts. It also gives the project a
practical sandbox boundary for filesystem, process, network, and secret access.

Native trusted modules remain useful for features that need deep platform
integration or performance, such as terminal integration, git backends, shell
interop, or provider-specific SDKs. They are not treated as marketplace-safe by
default and should be built in or explicitly trusted by the user.

## Manifest-First Registry

Every extension declares its shape before it runs. The manifest contains:

- Runtime and entrypoint.
- Permissions.
- Command contributions.
- Event contributions.
- UI contributions.
- Preview and metadata providers.
- Converter actions.
- Themes and icon packs.
- Filesystem/provider integrations.
- Sync participation.

The registry validates manifests before activation. It rejects unsupported schema
versions, invalid identifiers, duplicate command keys, unsafe package paths, and
contributions that require undeclared permissions.

This makes extension discovery cheap and safe: the app can list commands,
panels, themes, and provider options without loading extension code.

## Semantic Output Data Plane

Extensions should improve the core file-manager experience by producing
structured semantic data. They should not send desktop-specific widget trees,
absolute colors, layout instructions, or Iced/web-specific rendering commands.

A git extension is the reference case. It should be able to observe the current
directory or visible files, compute git state, and publish data like:

- file `NodeId` or path identity
- source extension id, such as `git`
- semantic state, such as modified, added, deleted, untracked, ignored,
  conflicted, or clean
- optional badge text, such as `M` or `A`
- optional theme token, such as `git.modified`
- optional tooltip or status text
- invalidation scope when repository state changes

`filer-core` routes that data as events. `filer-app`, a future web client, or
another frontend decides how to render it. One client might show a colored
filename and badge; another might show only a compact marker. The contract is
the meaning, not the presentation.

This model also applies to preview providers, metadata providers, converter
actions, command palette entries, context menu actions, panels, provider status,
and diagnostics. Core owns routing, cancellation, permission checks, session
scope, and invalidation. Clients own visual presentation.

The ecosystem should not grow every surface at once. After git decorations work,
additional surfaces can be added one at a time when they have a concrete product
use case and do not compromise the file-manager kernel.

Early client-facing outputs should be limited to row decorations, status badges,
context or command actions, preview payloads, and metadata payloads. Arbitrary
extension tabs, popups, panels, and layout control are intentionally postponed
because they can turn the app into a plugin-rendered UI shell before the core
data plane is proven.

Extension caching is allowed, but invalidation must be part of the core
contract. Extension-owned caches that cannot respond to directory changes, watch
events, manual refresh, branch/index changes, or timeouts will produce stale UI
and should not be treated as reliable.

## Programmer Helper Boundary

Programmer-oriented extensions should make Filer a better file reader and file
operator, not a compiler, debugger, or IDE. Good fits include git decorations,
external editor and terminal launchers, lightweight metadata, previews,
converters, and provider integrations. Continuous compilation, language-server
diagnostics, debugging, and build-system ownership are outside the near-term
extension model.

## Permission Model

Permissions are explicit and coarse enough for users to understand:

- Filesystem read/write.
- Watch and search.
- Network access.
- Process execution.
- Secret access.
- UI contribution.
- Provider integration.
- Profile sync participation.

Contribution-level required permissions must be covered by the manifest-level
permission set. This keeps the registry from accepting extensions that quietly
request power only when a command is used.

The initial crate validates declarations only. Runtime enforcement belongs to the
future host bridge, which must map these permissions to scoped filesystem access,
session policy checks, process/network gates, and secret storage.

## Wire-Safe Extension Contracts

The existing core has `Command::Extension` with an `Arc<dyn Any>` payload. That
is useful for trusted in-process Rust modules, but it is not suitable as the
public ecosystem boundary because it cannot cross process, web, package, or sync
boundaries.

The ecosystem contract is designed around serializable data. Future app/core/web
bridges should use versioned extension envelopes for public extension traffic,
while keeping `Arc<dyn Any>` as an internal escape hatch for trusted native
integration.

The wire-safe contract should cover both commands sent to extensions and outputs
emitted by extensions. This keeps extension-produced file decorations, badges,
metadata, previews, panels, and action state portable across desktop, web, and
server transports.

## Package And Unpack Model

`.filerpack` is the planned package format. The current crate defines package
metadata, package file entries, checksums, and signature metadata placeholders.

The design choices are:

- Package paths must be relative and safe to unpack.
- Hashes are required for package file validation.
- Signature fields are present in the model before signing is implemented.
- Packages should install disabled by default unless they are built-in or
  explicitly trusted.
- Portable packages should not include secrets by default.

Pack/unpack tooling will later use this contract to build deterministic packages,
verify file hashes, review permissions, and install into the local profile.

## Local Profile Sync Model

Local app configuration and ecosystem profile state are separate concerns.
Bookmarks, recent paths, theme, layout, panel visibility, density, and basic
sort/group preferences should stay in simple app-owned config until there is a
real sync requirement.

The ecosystem profile model is for extension and platform state. It is changed
by append-only operations such as install, remove, enable, disable, extension
settings updates, theme/icon-pack changes, provider profile additions, and
workspace updates.

This is chosen over a server-first design because the desktop app should remain
fully useful without a server. The same operation log can later be streamed by
`filer-server` or replayed by web/mobile clients.

The v1 profile operation model is intentionally simple:

- Operations are idempotent by operation id.
- Unsupported future operations can be ignored or preserved by later transport
  layers.
- Secrets are not part of normal portable sync; they should live in the OS
  keychain or encrypted local profile storage.

## Integration Boundaries

`filer-core` remains the execution authority. It owns sessions, providers,
actors, operations, search, preview, and event routing. Extensions should not
bypass these paths.

`filer-app` renders ecosystem contributions. It should discover extension
commands, menus, panels, badges, themes, icon packs, and provider profiles from
validated manifests and registry state.

The app should render live extension data from core events rather than letting
extensions mutate UI state directly. This keeps app behavior testable and keeps
extensions portable across clients.

Future web/server crates should reuse the same manifest and profile operation
types. They should transport operations and extension events, not invent a
separate ecosystem protocol.

## Security Defaults

The safe default behavior is:

- Installing a package does not automatically enable it.
- Enabling an extension requires reviewing permissions.
- Third-party extensions should use WASM unless explicitly trusted.
- Native trusted extensions are treated like installed software, not normal
  marketplace plugins.
- Secrets do not leave the local machine unless an explicit encrypted export
  flow is implemented.
- Filesystem access must be scoped through session policy and provider
  capabilities.

## Current Limitations

- No WASM runtime exists yet.
- No native plugin loader exists yet.
- No app extension manager UI exists yet.
- No pack/unpack CLI exists yet.
- No profile sync transport exists yet.
- Permission validation is declarative only; runtime enforcement is future work.
- Live extension output contracts for file decorations, badges, panels, and
  action state are planned but not implemented yet.
- Git file decoration has not yet been implemented as the first vertical slice.

These limits are intentional for the first milestone. The current crate is the
contract foundation that the runtime, app UI, and sync layers will build on.
