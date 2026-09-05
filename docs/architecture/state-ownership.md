# State Ownership

Filer keeps state in three crates, and each crate owns a distinct kind. When a piece of state lands in the wrong crate, the application UI and the portable profile drift into coupling that is hard to undo. This document defines who owns what so you can place new state correctly the first time. It extends the `CORE-LIBRARY` and `SESSION-BOUNDARY` rules in `invariants.md`.

The three owners are `filer-app` for application UI state, `filer-core` for runtime and session state, and `filer-ecosystem` for portable profile and sync state. The rest of this document describes each boundary and the one rule that matters most: provider secrets never leave the runtime.

## Application UI state is app-owned

`filer-app` owns everything tied to how the desktop application looks and what the user pinned. That is the `Config` struct in `filer-app/src/config.rs`: `bookmarks`, `recent_paths`, `preview_visible`, `sidebar_width`, `preview_width`, and `theme_mode`. The app loads and saves this file under the user's config directory.

`filer-core` must not hold any of this. The `CORE-LIBRARY` rule already forbids `filer-core` from depending on GUI frameworks or app-specific state, and theme, layout, and bookmarks are app-specific. If a frontend needs new presentation state, it belongs in the app config, not in core.

## Core owns session and runtime state

`filer-core` owns the state that exists only while the application runs. A Session represents an independent client activity context. Closing its view does not end accepted file operations.

Session identity lives in `SessionId` (`filer-core/src/model/session.rs`). The `SessionManager` in `filer-core/src/api/session_manager.rs` holds the live sessions in a concurrent map and owns their lifecycle from handshake to teardown. Each `Session` carries its own `NavigatorState` (`filer-core/src/modules/navigation/navigator.rs`), so navigation history and selection stay scoped to one client. This is the `SESSION-BOUNDARY` rule in practice: state that belongs to user activity carries session identity.

`NavState` is the serializable snapshot of navigation that crosses the wire to a client. `NavigatorState` is the full mutable state that stays in core. Keep that split. Send `NavState`, do not leak `NavigatorState`.

Authorization remains a future Core concern; authentication belongs to the transport. The supported native model uses `AllowAll` and the OS user's filesystem permissions. Restricted-Session enforcement is deferred: the existing `SessionPolicy` types do not establish that command execution enforces their rules. Future Core policy may narrow OS-granted access. Do not move authentication tokens or transport credentials into Core.

Per-session view settings also live in core as `PipelineConfig` (`filer-core/src/pipeline/config.rs`), which holds sort, filter, and group choices. This config is small and serializable so it can travel between a frontend and core. It describes how to present a directory, not how the app window looks, so it stays in core rather than app config.

The accepted [runtime lifecycle](../adr/0001-core-runtime-lifecycle.md) assigns
mutation admission, per-Session FIFO queues, failure pauses, and completion to
Core. The client owns the recovery UI and keeps consuming events while Core
finishes accepted work. SessionDestroyed will mean completed cleanup, not receipt
of a close request; REL-008 owns that change from current behavior.

Event delivery currently uses one shared runtime stream. The client bridge owns
its single consumer and distributes events by Session identity. Separate Session
streams are deferred, so shared backpressure remains a limitation.

## Provider references and the secrets boundary

A location names its provider through `ProviderRef` in `filer-core/src/model/location.rs`. It has three forms. `Local` is the operating system filesystem. `Profile(name)` points at a named provider profile resolved at runtime. `Ephemeral(name)` is a session-local identity that is valid only for runtime lookup and must not be persisted unless its descriptor can be rebuilt next session.

Provider secrets live only in `filer-core` runtime configs and only while a session runs. Future provider implementations must resolve credentials from a runtime credential source, then keep those values out of portable profile state and sync state. Runtime provider configs may hold credentials in memory while a session needs them, but they must not become the durable identity for a profile.

The rule that protects users: a secret must never enter portable or sync state. The portable side references a provider by id only, never by credential. When you add a provider or a new credential field, resolve the secret at runtime and keep it out of anything that serializes to disk for sync.

## Portable profile and future sync

`filer-ecosystem` owns the state that syncs across a user's devices. It is a runtime-free crate of wire-safe contracts. `ProfileState` in `filer-ecosystem/src/lib.rs` holds the installed and enabled extensions, extension `settings`, the selected `theme_id`, `workspaces`, and `provider_profiles`. That `provider_profiles` map is `profile_id` to `scheme` only. It records which provider a profile uses, never the credential to reach it. This is how the secrets boundary holds across sync.

`SyncParticipation` declares whether an extension syncs its settings and state. Sync ownership stays in `filer-ecosystem` and the app layer that drives it. Do not move UI persistence into `filer-core` to make it syncable. App UI config that should sync belongs in the profile model in `filer-ecosystem`, reached through the app, not by promoting `filer-app/src/config.rs` fields into core.
