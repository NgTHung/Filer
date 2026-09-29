# filer-ecosystem

`filer-ecosystem` defines the data that Filer extensions, packages, and profile sync exchange. `filer-core`, `filer-app`, web clients, and package tools all read the same manifests and package metadata, so an extension declared once works in every client. The crate holds only serializable types and validation. It runs no extension code, loads no plugins, renders no UI, and touches no files, so any process can depend on it without pulling in a runtime.

## What it contains

- `ExtensionManifest` declares an extension's runtime, permissions, commands, events, UI surfaces, previews, metadata providers, converters, themes, icon packs, file providers, and sync participation.
- `validate_manifest` checks the schema version, required fields, identifiers, and that every contribution's permission is declared. `EcosystemRegistry` holds validated manifests and rejects duplicate extension ids and command keys.
- `ExtensionPackage` describes a `.filerpack` archive with its files and signature. `validate_package` rejects unsafe or duplicate paths and malformed SHA-256 digests.
- `ProfileOperation` and `ProfileState` describe profile changes for pack, unpack, and sync workflows.

Profile state covers extension and package state, provider profiles, and workspace state. App settings such as bookmarks, recent paths, theme, and layout stay in the app's own configuration, because they do not need to travel between machines or clients.

## Extension model

An extension has two planes. The declaration plane is the manifest: what the extension offers and which permissions it needs. The data plane is what a running extension publishes through core, such as file decorations, status badges, action state, metadata updates, and preview payloads.

Extensions publish meaning, not widgets. A Git extension reports that a file is modified or added, and each client picks how to display it. Early UI output stays narrow: row decorations, status badges, command actions, previews, and metadata. Arbitrary tabs, popups, and layout control stay out, because each one would tie an extension to one client's UI.

The runtime is meant to be hybrid. Third-party extensions would run as sandboxed WASM, and built-in or explicitly trusted integrations as native modules. Until sandboxing, package installation, and permission enforcement exist, an in-process host is a trusted add-on model and makes no marketplace safety promise.

## More detail

- [DESIGN.md](DESIGN.md) explains the architecture choices and tradeoffs.
- [ROADMAP.md](ROADMAP.md) lists the ecosystem features and their order.
