# Filer App

`filer-app` is a desktop client for `filer-core`, not a second filesystem engine. The app owns presentation, interaction, and desktop integration while core remains the authority for navigation, directory data, search, previews, operations, providers, and runtime sessions. This contract keeps the application design stable while the UI framework remains replaceable.

This document defines the architecture for the filer-app rewrite. It does not preserve the current app implementation, and it does not select a UI framework.

## Status and scope

This is the framework-neutral contract for the new desktop application. Candidate implementations must follow the same state, data-flow, reliability, performance, and testing requirements so the framework evaluation compares equivalent clients.

The design covers:

- application and core ownership boundaries
- runtime state and persistence
- command and event flow
- large-directory paging and virtualization
- selection, navigation, search, preview, and operations
- desktop platform services
- accessibility and international input
- failure recovery, diagnostics, and testing
- the adapter contract required from any UI framework

Feature-specific product behavior belongs in `.tasks/app` after a framework is selected. The architecture only fixes boundaries that would be expensive to change later.

## Hard rules

The rewrite must preserve these rules.

1. `filer-core` never depends on the application or its UI framework.
2. Framework types never enter the controller, application model, persistence schema, or core bridge.
3. New application code consumes Location-native commands and events. It does not build new behavior on `NodeId`, `FileNode`, or path compatibility routes.
4. Filesystem behavior goes through `filer-core`. Direct filesystem access is limited to app configuration, framework assets, and explicit operating-system integration.
5. The UI thread never performs blocking filesystem, provider, preview, metadata, search, or operation work.
6. One controller is the only writer of application state. Background work reports typed events to it.
7. A frame performs work proportional to visible content and received changes, not the total logical directory size.
8. Every asynchronous result is checked against its session and correlation identity before it can change visible state.
9. Destructive actions remain pending until core reports the result. The app does not pretend an operation succeeded.
10. Accessibility, keyboard use, international text input, and high-DPI behavior are release requirements, not optional polish.

These rules extend `CORE-LIBRARY`, `PROVIDER-ACCESS`, `SESSION-BOUNDARY`, `ACTOR-LONG-WORK`, and `PIPELINE-TRANSFORMS` from [Architecture Invariants](invariants.md).

## Ownership boundaries

Filer separates runtime mechanics, application behavior, and framework rendering.

| Owner | Responsibilities | Must not own |
| --- | --- | --- |
| `filer-core` | Sessions, provider resolution, navigation state, directory paging, pipeline transforms, search, preview, metadata, watching, operations, progress, and structured errors | Window geometry, themes, panels, focus, widget state, or desktop shell presentation |
| App controller | Intent reduction, tab coordination, visible state, selection, pending requests, error presentation, persistence scheduling, and platform-service coordination | Provider I/O, directory scans, operation execution, or framework widgets |
| Platform services | Native dialogs, clipboard, drag-and-drop, shell launching, notifications, theme observation, and window persistence | Directory truth or provider policy |
| UI adapter | Widgets, layout, rendering, accessibility projection, input translation, wakeups, and framework-specific tests | Business rules, core routing, persistent data formats, or authoritative file state |

App-owned and core-owned state must continue to follow [State Ownership](state-ownership.md). Provider secrets never enter app configuration, logs, drag payloads, or framework state.

## Logical modules

The final crate should be split by responsibility rather than screen. Names may change, but the boundaries must remain.

| Module | Purpose |
| --- | --- |
| `bootstrap` | Process startup, tracing, dependency construction, shutdown, and fatal startup reporting |
| `controller` | Reduces user intents and external events into state changes and effects |
| `model` | Framework-free application, window, workspace, tab, directory, selection, preview, operation, and overlay state |
| `intent` | Typed user actions independent of widgets and input devices |
| `effect` | Commands for core, persistence, platform services, timers, and framework wakeups |
| `core_bridge` | Session lifecycle, command submission, event reception, batching, and correlation checks |
| `platform` | Interfaces for desktop services with platform-specific implementations |
| `persistence` | Versioned configuration loading, validation, migration, and atomic saving |
| `presentation` | Derived labels, row projections, enabled states, semantic roles, and formatting |
| `ui` | The selected framework adapter and no framework-neutral logic |

Keep modules under the repository size guidance. Split model, controller, and platform code by concept before any file approaches 700 lines.

## Dependency direction

Dependencies point inward toward framework-free policy.

- `model`, `intent`, and `presentation` use Rust types and selected public `filer-core` model types.
- `controller` depends on `model`, `intent`, and abstract effect sinks.
- `core_bridge`, `platform`, and `persistence` implement effect boundaries.
- `ui` reads presentation state and emits intents.
- No inner module imports the selected UI framework.

The controller must be testable without creating a window, graphics device, event loop, or real filesystem.

## Application state

The application model is retained even when the selected framework uses immediate-mode rendering. Widgets are a projection of this model, not the source of truth.

The root state contains:

- process lifecycle and startup status
- zero or more windows
- global preferences and recent locations
- active operations and notifications
- platform capability state
- recoverable and fatal diagnostics

Each window contains:

- window identity and persisted geometry key
- active workspace and tab identity
- sidebar, preview, details, and overlay presentation state
- focus intent and command routing context
- per-window theme and scale observations

Each navigation tab contains:

- its core `SessionId`
- current `LocationRef` and display label
- current core navigation snapshot
- `PipelineConfig`
- directory view generation and paged row store
- selection, anchor, and focused row
- search and preview state
- latest request identities by request class
- pending page, metadata, preview, and refresh effects

Transient hover, animation, and pointer-capture data may stay in the framework adapter. Any state that affects commands, persistence, keyboard behavior, accessibility, or tests belongs in the application model.

## Identity

`LocationRef` is the application identity for locations and rows. The app may use a compact internal key derived from a full or descriptor reference, but that key must never replace the original provider-aware identity at a core boundary.

The app must not persist an ID-only `LocationRef`. Persist a reconstructable descriptor. Do not persist an ephemeral provider reference unless its provider descriptor can be restored without a secret.

Row position is never identity. Sorting, grouping, paging, insertions, and deletions may change positions without changing the selected location.

## Sessions and tabs

Each independent navigation context uses its own core session. A single-tab window therefore owns one session. Additional tabs own additional sessions so their history, pipeline state, cancellation, and events cannot interfere.

The app keeps an explicit mapping from `SessionId` to window and tab identity. An event with an unknown or destroyed session is logged and discarded.

Tab closure follows this order:

1. mark the tab as closing and stop accepting new intents
2. cancel tab-owned search and preview work
3. request core session destruction
4. remove the tab after session teardown or a bounded shutdown fallback

Application shutdown stops new commands, persists valid app state, destroys sessions, drains terminal events for a bounded interval, and calls core shutdown. Shutdown errors are reported and must not be ignored.

## Unidirectional data flow

All interaction follows one cycle:

1. The framework translates input into an `Intent`.
2. The controller validates the intent against current state.
3. The controller updates immediate app-owned state and returns `Effect` values.
4. Effect handlers call core, persistence, platform services, or framework wakeups.
5. Results return as typed external events.
6. The controller correlates and reduces those events.
7. The framework renders the resulting presentation state.

Intents describe meaning, not controls. Examples include `NavigateTo`, `RequestNextPage`, `SetSort`, `ExtendSelection`, `BeginRename`, `ConfirmDelete`, and `RetryRequest`. Names such as `SidebarButtonClicked` or framework event types must not cross into the controller.

Reducers perform no I/O. Effects perform no unsynchronized state mutation.

## Core bridge

The core bridge owns the mechanical connection to `CoreHandle`:

- create and destroy sessions
- allocate request and operation IDs
- send Location-native `Command` values
- receive `Event` values without blocking the UI thread
- wake the framework event loop when work arrives
- preserve terminal-event ordering
- apply bounded batching and backpressure
- expose transport health and shutdown state

The bridge must not poll continuously at idle. Use the framework's event-loop proxy, subscription, channel wakeup, signal, or equivalent mechanism.

Core events enter the controller through an app-owned enum. The bridge may normalize compatibility-free event shapes, but it must retain session, request, operation, location, completion, and error data.

## Correlation and stale-result rejection

Every navigation, refresh, page, search, preview, and metadata request records its `RequestId`. Every operation records its `OperationId`. The tab also increments a local view generation whenever navigation, pipeline settings, or an authoritative refresh replaces the visible dataset.

An incoming result is accepted only when:

- its session maps to a live tab
- its request is still active for that request class
- its location matches the expected target where the event supplies one
- its view generation is still current
- its page cursor and window are consistent with the pending page request

Cancellation is not a substitute for correlation. A cancelled worker may race with delivery, so stale results must still be rejected.

Terminal events clear matching pending state exactly once. Duplicate or late terminal events are logged at a low severity and do not corrupt state.

## Directory view model

The directory view is a paged logical collection, not a widget list. It stores core-produced groups and `NodeEntry` rows in immutable or append-only page chunks where practical. Publishing a new frame must not clone the entire directory.

The model records:

- parent `LocationRef`
- current view generation
- current `PipelineConfig`
- page chunks and their zero-based start indices
- loaded row count and optional provider total
- next cursor and completion state
- pending page request
- group boundaries
- row identity lookup for loaded entries
- recoverable page error state

`DirectoryPageState.start_index`, `loaded_count`, `next_cursor`, and `complete` are authoritative paging inputs. The app validates that an appended page does not create an unexplained gap or overlap. An inconsistent page starts a recoverable refresh rather than silently duplicating rows.

The first page replaces the previous generation. Later pages extend the same generation. Navigation, sort, grouping, filter, and refresh create a new generation and invalidate outstanding page results from the old one.

## Virtualization contract

The UI adapter receives a logical row count and a way to project a row or group header by index. It asks only for the visible range plus a small overscan range.

The adapter must not:

- create one widget per logical entry
- format every loaded row during each frame
- clone the loaded row collection to render
- measure off-screen row content during normal scrolling
- request pages solely because a repaint occurred

Formatting and icon lookup use caches keyed by row identity and relevant metadata. Cache entries are invalidated by row changes, theme or scale changes, and bounded memory policy.

When the visible range approaches the loaded boundary, the controller may request one next page. Duplicate next-page requests are suppressed. Prefetch thresholds are configuration values measured by the benchmark, not hidden framework constants.

## Selection

Selection is app-owned and keyed by `LocationRef`. It records:

- selected loaded identities
- focused identity
- range anchor identity
- last input modality where it changes focus presentation

Range selection resolves through the current loaded presentation order. A generation change retains only identities that are still present when the operation is unambiguous. Navigation clears directory selection unless a restoration flow explicitly supplies a matching location and generation.

The initial rewrite defines Select All over loaded entries. Selecting unloaded provider results would require a separate core selection contract and must not be simulated by the UI.

Selection changes never trigger metadata or preview work for every selected row. The focused row drives single-item detail and preview requests. Aggregate selection summaries use already loaded data and bounded work.

## Navigation

The controller maps navigation intents to core navigation commands. Core navigation state remains authoritative for current location, back history, and forward history.

The address editor uses a separate draft string. Editing the draft does not change current location. Submission parses the value into a provider-aware location before sending a command. Invalid input remains editable and receives a local validation error.

Breadcrumbs are a presentation of the current location descriptor and segments. Breadcrumb controls emit locations, not local paths, so archives and future providers follow the same route.

Refresh creates a new directory generation while keeping the old rows visible with a refreshing state until the replacement first page arrives. A failed refresh keeps the last valid rows and presents retry information.

## Pipeline settings

Sort, filter, hidden-file, and grouping choices are represented by `PipelineConfig`. The UI renders available choices and emits a complete proposed configuration. Core remains the authority for transforms and paging mode.

Changing pipeline settings:

1. records the proposed configuration
2. creates a new request and generation
3. cancels incompatible outstanding directory work
4. keeps the prior result visible as stale
5. replaces it when the new first result arrives

The app does not re-sort or re-group core output to make one framework easier to use.

## Filesystem changes

Watcher events are hints scoped by session and location. The app applies a direct row update only when the event contains enough provider-aware information to do so without guessing. Otherwise it schedules a coalesced refresh.

Bursts are coalesced over a short bounded window. Deletion of the focused or selected location clears affected UI state. Rename handling preserves selection only when the event or operation result identifies the new location without inference.

Event coalescing must preserve terminal operation, error, and session events. It must not reorder changes across an authoritative refresh boundary.

## Search

Search state is separate from directory paging even when results share the same row presentation.

Each search records:

- query draft and submitted query
- root `LocationRef`
- request identity and status
- append-only result chunks
- completion and recoverable error state
- selection and focused result

Submitting a new search cancels the previous search for that tab and rejects its late results. Empty drafts do not trigger search. Search result rendering follows the same virtualization and row projection rules as directories.

## Preview and metadata

Preview, basic metadata, and extended metadata are independent request classes. Focusing a row schedules debounced work. Moving focus cancels obsolete work and retains correlation checks.

The model distinguishes not requested, pending, ready, unavailable, failed, and cancelled states. A previous item's preview must never appear under a new selection while its replacement is loading.

Preview payload decoding, image upload, and large text preparation stay outside the reducer. GPU resource creation stays in the UI adapter. The app model retains provider identity and framework-neutral preview data or a bounded resource handle.

## Operations

Copy, move, delete, rename, create-file, and create-folder actions are effects against Location-native core commands. The controller checks known capability state before enabling or dispatching them, while core remains the final authority.

Every operation records:

- operation and request identities
- kind, sources, and destination
- originating tab and session
- pending confirmation or conflict state
- progress snapshot
- completion or structured error
- affected locations returned by core

Destructive confirmations are app-owned overlays. Conflict behavior comes from core contracts and provider guarantees. The UI must not invent atomicity, undo, trash, or cross-provider guarantees.

Pending operations may decorate rows, but they do not remove or rename authoritative rows before core completion or a matching filesystem change. On success, affected views refresh or reconcile from explicit affected locations. On failure, the original view remains valid.

## Clipboard and drag-and-drop

Clipboard and drag-and-drop are platform services expressed as framework-neutral payloads. Internal payloads carry `LocationRef` values and an allowed operation. External payloads carry operating-system paths only when the source or destination is representable as a direct local path.

The platform adapter must support multiple items. Unsupported provider-to-shell transfers are disabled with a reason. Dropped items become a copy, move, navigate, or open intent only after target and modifier validation.

Clipboard ownership changes, partial payloads, invalid paths, and unsupported formats return explicit outcomes. They are not ignored silently.

## Platform services

Desktop integration is isolated behind interfaces so framework selection does not change controller behavior.

Required services include:

- open and save dialogs where product flows need them
- clipboard read and write
- inbound and outbound multi-item drag-and-drop
- open with the operating-system default application
- reveal or open a terminal where supported by approved tasks
- notifications and attention requests
- system theme and scale observation
- window geometry and monitor information
- app configuration and cache directories
- file-type icons or thumbnails where they are platform-owned

Each service declares platform support and returns a typed unsupported result. Platform-specific code must not spread into views or reducers.

## Persistence

Application configuration is versioned and app-owned. It may contain bookmarks, recent reconstructable locations, window geometry, panel visibility and sizes, theme preference, density, and other presentation settings approved by app tasks.

Persistence follows these rules:

- validate loaded values and apply bounded defaults
- migrate known older versions explicitly
- preserve no provider secrets
- persist reconstructable location descriptors, not runtime-only IDs
- write to a temporary file and replace atomically where the platform permits
- surface load and save failures through tracing and a nonfatal diagnostic
- debounce repeated layout writes
- never perform disk I/O in the reducer or render callback

Unknown newer versions fail safely without overwriting the source file.

## Accessibility and input

The application defines semantics independently of widgets. The adapter maps them to the framework and operating-system accessibility APIs.

Required semantics include:

- named windows, regions, toolbars, navigation controls, dialogs, and status messages
- list, table, tree, row, cell, header, button, checkbox, text input, progress, and menu roles
- selected, focused, expanded, disabled, busy, checked, and invalid states
- row names that do not depend on icons or color
- announcements for navigation completion, recoverable errors, and completed operations

All primary workflows must be keyboard accessible. Focus order is deterministic and focus remains visible. Multi-selection supports the platform's standard modifiers.

Text input must support composition, candidate windows, Unicode grapheme movement, bidirectional text, font fallback, clipboard text, and filenames that are not valid UTF-8 where the platform and core expose them. Candidate frameworks must be tested with Vietnamese, Chinese, Arabic, emoji, combining marks, and long names.

## Rendering and themes

Presentation code derives semantic style tokens such as surface, text, selection, focus, danger, spacing, density, and icon size. Framework-native color, brush, shader, or style objects stay in the adapter.

System light and dark themes are supported. High-contrast and reduced-motion signals are honored when exposed by the platform. Selection, focus, errors, and operation state must remain distinguishable without relying on color alone.

Custom GPU rendering is an adapter capability for previews or specialized surfaces. Vulkan, OpenGL, Direct3D, Metal, and wgpu are renderer choices, not replacements for the application architecture.

## Errors and recovery

Errors retain core `kind`, `code`, target, context, recoverability, session, request, and operation correlation. The controller maps them to one of four presentation levels:

| Level | Use |
| --- | --- |
| Inline | A row, field, preview, page, or address error with a local recovery action |
| Notification | A completed background failure that does not block the current view |
| Modal | A decision or failure that blocks a user-requested destructive flow |
| Fatal | Startup or runtime loss that prevents safe continued use |

Recoverable errors expose retry only when the original intent can be reconstructed safely. Retrying allocates a new request identity. Fatal errors preserve diagnostics and permit clean shutdown.

The app must not convert an error into an empty directory, missing preview, or successful operation unless the contract explicitly defines that outcome.

## Event batching and responsiveness

The bridge may batch events to keep wakeups bounded. Each batch is processed within a time budget, then control returns to the event loop before more work is reduced.

Safe coalescing candidates include superseded nonterminal progress updates and repeated refresh hints for the same generation. Unsafe candidates include operation completion, session lifecycle, structured errors, first-page replacement, and events separated by a generation change.

If the UI cannot keep up, the app reports queue depth and processing time. It must not grow an unbounded framework-side queue or drop terminal state.

## Performance contract

Framework candidates are measured in release builds on recorded hardware and operating-system versions. The same scenarios, data, fonts, icons, window size, and enabled features apply to every candidate.

Required properties are:

- layout and rendering cost is proportional to visible rows plus overscan
- directory state grows with loaded rows, not provider-reported total rows
- no full row-array clone occurs during a frame, selection change, or page append
- the event loop performs no busy polling while idle
- the first core page becomes visible within one normal frame after reduction
- rapid scrolling does not trigger duplicate page requests
- thumbnail and metadata arrival does not stall input
- a stale result cannot replace a newer view

Provisional targets on the recorded reference machine are:

- p95 scrolling frame time at or below 16.7 ms
- p99 scrolling frame time at or below 33.3 ms
- p95 input-to-visible-state latency at or below 50 ms when no core I/O is required
- ordinary event reduction uses no more than 4 ms of one frame before yielding
- idle UI wakeups approach zero when no timer, animation, or external event is active

Results outside a target are not hidden by averages. The evaluation records the cause, repeatability, and whether the framework or app design owns it.

## Framework adapter contract

A candidate framework is acceptable only if its adapter can provide:

- a virtualized details list with sortable headers and reusable row presentation
- variable visibility for group headers without materializing all rows as widgets
- event-loop wakeup from the core bridge
- multiple windows or a documented product constraint
- keyboard focus, shortcuts, pointer input, menus, tooltips, and dialogs
- international text input and clipboard support
- inbound and outbound multi-item drag-and-drop
- an operating-system accessibility tree with required roles and states
- high-DPI and live scale changes
- custom preview rendering without contaminating the controller
- deterministic interaction tests and captured failure artifacts
- release builds on supported desktop platforms

Missing capabilities must be demonstrated and recorded. A candidate cannot receive a passing result through an unimplemented placeholder.

## Testing strategy

Tests follow the repository TDD rules and live outside production modules.

### Controller tests

Pure tests cover intent validation, reduction, effect emission, session routing, request correlation, stale-event rejection, refresh replacement, operation state, error mapping, and persistence scheduling.

### Model tests

Model tests cover page insertion, gap and overlap detection, group projection, selection across changes, identity stability, view generations, and bounded caches. Property tests are appropriate for page sequences and selection invariants.

### Core bridge tests

A fake core port verifies command construction, Location-native routing, event batching, wakeups, cancellation, channel closure, and bounded shutdown. Contract tests use the real public core API for one vertical slice without depending on a UI framework.

### Platform tests

Platform adapters receive focused tests for clipboard formats, multi-item drag-and-drop, atomic configuration replacement, unsupported outcomes, theme and scale changes, and shell error propagation.

### Framework tests

Each candidate implements the same scripted workflows: navigate, page, scroll, sort, filter, select, range-select, rename, search, preview, copy, move, delete confirmation, drag-and-drop, keyboard navigation, IME input, scale change, and error recovery.

Tests must inspect semantic state where possible. Screenshots supplement behavior tests but do not replace them.

### Performance tests

The evaluation includes deterministic synthetic providers for 100, 10,000, 100,000, and 1,000,000 logical entries. Scenarios record startup, first useful frame, page append, steady scrolling, burst updates, sort and filter replacement, selection, thumbnail arrival, idle resource use, memory, binary size, and build time.

Raw samples, build profile, dependency revision, hardware, display scale, and runner configuration are retained with the result.

## Diagnostics

Tracing spans connect user intent, session, request, operation, location, bridge queue, reduction, and visible completion. Location logging uses redacted provider-safe formatting.

Required counters include:

- bridge queue depth and dropped nonterminal updates
- events processed and time spent per batch
- current directory generation and loaded row count
- page requests issued, suppressed, completed, cancelled, and rejected as stale
- cache entry counts and evictions
- frame and input latency samples in benchmark builds

Diagnostics must not record provider credentials, clipboard contents, arbitrary file contents, or preview payloads.

## Framework evaluation boundary

Framework spikes may share framework-free fixtures, scripted intents, synthetic core events, expected semantic snapshots, and measurement output. They must not share framework-specific rendering code.

Each spike records:

- exact released version or pinned source revision
- supported and tested platforms
- adapter size and any required unsafe or foreign-language boundary
- missing capabilities and local patches
- performance samples under the common scenarios
- testing and accessibility evidence
- packaging, licensing, and maintenance risks

The final decision compares measured behavior and maintenance cost. Repository popularity or a small demo is not evidence that a framework meets the contract.

## Delivery stages

The rewrite should land in reviewable stages after the framework decision.

1. Build and test the framework-free model, controller, effects, and fake ports.
2. Add the selected framework adapter with one window, one tab, navigation, and a virtualized directory view.
3. Connect search, preview, metadata, watching, and operation progress through the same bridge.
4. Add platform clipboard, drag-and-drop, dialogs, shell integration, accessibility, and persistence.
5. Prove performance, failure recovery, and UI behavior on supported platforms.
6. Remove the deprecated app implementation and unused dependencies only after the replacement reaches contract parity.

Each stage must compile and test independently. The rewrite must not keep two application state authorities alive behind different screens.

## Decision record

The chosen framework and rejected candidates will be recorded after the common evaluation epics complete. That record must identify measured evidence, accepted limitations, required upstream work, dependency pinning, licensing, and the fallback plan if the chosen framework cannot meet a release gate.

