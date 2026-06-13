# Architecture Fit Review (CORE-005)

Does the current filer-core design carry the project's ambitions, or will it collapse as
features land? This report maps each stated ambition to the structures meant to support it,
ranks the structural risks, and lists follow-up candidates. It is review-only.

Evidence is cited as `path:line` against the crate at the time of review.

## Ambitions under test

From `README.md`, `ROADMAP.md`, and `docs/architecture/invariants.md`:

1. **Fast, non-blocking navigation** of very large local directories (the `C:\Windows\System32` proof target).
2. **Cross-client core** — one core behind desktop (Iced) today, web/server later, with identical behavior.
3. **Pluggable providers** — local now; S3, WebDAV, SFTP, K8s, FUSE, and archives without core churn.
4. **Semantic extensions** — git decorations and similar, emitting data not UI, across a transport boundary.
5. **Reliability** — cancellation, stale-result suppression, structured errors, freshness under mutation.

## Verdict

**The architecture stands. It will not collapse under its own weight if three known gaps are 
closed before the features that depend on them land.** The core's load-bearing decisions are
sound: the addressing model is additive and serializable, long work is actor-isolated with
per-session cancellation, and directory transformation flows through one pipeline contract.
The risk is not structural rot. It is three deferred contracts whose cost rises sharply the
later they are retrofitted, because each touches a wide call surface.

## What is sound (carries the ambition)

- **Location addressing model** (`model/location.rs`). `LocationDescriptor` separates identity
  (scheme, provider, root, segments) from display, and `LocationId` hashes identity only
  (`location.rs:189`). `LocationRef` carries Id, Descriptor, or Full, so callers choose
  compactness vs reconstructability. The model is `Serialize`/`Deserialize` throughout, so it
  is already transport-ready. `route()` (`location.rs:155`) classifies into DirectPath,
  Segmented, or UnsupportedProvider, and `require_direct_path()` returns structured
  `CoreError`s rather than panicking on the not-yet-supported routes (`location.rs:220`). This
  is the right shape for ambitions 2, 3, and the archive part of 3 — the unsupported cases are
  represented, not assumed away.
- **Actor isolation + cancellation** (`actors/`, `modules/*`). Scan, navigate, search, preview,
  operations, and watch each sit behind actors with per-session cancellation, satisfying the
  ACTOR-LONG-WORK and SESSION-BOUNDARY invariants. This is what makes ambition 1 achievable.
- **Pipeline transform contract** (`pipeline/`). Filter, sort, and group flow through one
  `Pipeline` producing `GroupedNodes`, so presentation logic does not leak into providers.
- **Provider abstraction with honest fallbacks** (`vfs/provider.rs`). `FsProvider` defaults
  `list_page` to materialize-and-slice (`provider.rs:103`) but exposes `ProviderPaging::Native`
  so LocalFs can override for true cursor paging. Remote read paths degrade explicitly
  (`read_header` returns `Err` to fall back to extension-only MIME detection, `provider.rs:153`).
  The trait models capability honestly rather than pretending all providers are equal.

## Structural risks (ranked)

### R1 — Extension payload is in-process only (Critical for ambitions 2 + 4)

`Command::Extension` carries `payload: Arc<dyn std::any::Any + Send + Sync>`
(`api/commands.rs:340`), and `wire_commands.rs` explicitly rejects it with
`ExtensionUnsupported` because "type-erased extension payloads [stay] in process until a
wire-safe extension contract exists" (`api/wire_commands.rs:4`, `:99`, `:333`). Today an
extension cannot cross a process or transport boundary. The git-decoration proof target and
any web/server client both require a serializable semantic envelope. The longer the in-process
`Arc<dyn Any>` shape is the only extension path, the more app and module code binds to it, and
the more expensive the eventual envelope migration becomes.

This is already owned by **MODULES-001** (define semantic extension data plane) and
**PROTOCOL-001** (versioned transport). The architectural recommendation: design the
serializable extension envelope before building any second extension consumer on top of the
`Arc<dyn Any>` path, so there is exactly one migration, not many.

### R2 — No deadline/cancellation context in the provider trait (High for ambitions 1 + 3)

Every `FsProvider` method takes only `&self` and paths (`vfs/provider.rs:71-187`). There is no
deadline, timeout, or cancellation token parameter. A slow remote provider (S3, WebDAV, SFTP)
can block a scan, search, or preview indefinitely, which directly threatens the non-blocking
ambition the moment a non-local provider is wired in. Adding a deadline parameter later is a
wide change: it touches the trait, every provider impl, and every call site in the actors.

This is owned by **PROVIDER-001** (propagate provider timeout context). The recommendation:
land the timeout/cancellation context on the trait before the second provider ships, so remote
providers are born deadline-aware rather than retrofitted. CORE-009 should produce the concrete
trait-shape proposal.

### R3 — Segmented and unsupported routes are represented but not executed (Medium for ambition 3)

`route()` can produce `Segmented` (nested archives) and `UnsupportedProvider`, but
`require_direct_path()` turns both into errors (`location.rs:223-243`). The model is ready; the
execution is not. This is the correct order — represent first, execute later — so it is a
Medium risk, not a Critical one. The risk is only realized if archive/remote execution is bolted
on without routing through the existing `LocationRoute` classification, which would fork the
addressing logic. Owned in spirit by **VFS-001** (route segmented provider locations).

### R4 — Cursor stability under mutation is best-effort (Medium for ambitions 1 + 5)

The default `list_page` is offset-based over a fresh full listing (`provider.rs:103-130`), so
concurrent directory mutation between page requests can skip or duplicate rows. For the large-
directory proof target with a live watcher, this can surface as lost or doubled entries during
browse. The contract is defined and the limitation is known; closing it needs a mutation-stable
cursor design. CORE-008 (async/actor) and CORE-010 (model/pipeline) should jointly scope this.

## Cross-cutting observation

The three Critical/High risks (R1, R2, R3) are all **deferred contracts on wide surfaces**, and
all three are already on the 0.3.0 roadmap (MODULES-001, PROVIDER-001, VFS-001). The
architecture's health therefore depends less on the structures that exist and more on **sequencing**:
each contract must be defined before the first feature that would otherwise harden the wrong
shape around it. The current task graph reflects this ordering. The danger is schedule pressure
tempting a feature to ship on the in-process or deadline-less path "just for now."

## Follow-up task candidates

These are candidates for the CORE-013 remediation backlog, not new tasks created here.

- Define the serializable extension output envelope before a second extension consumer exists
  (feeds **MODULES-001**). Severity: Critical.
- Add deadline/cancellation context to `FsProvider` before the second provider ships
  (feeds **PROVIDER-001**; concrete shape from CORE-009). Severity: High.
- Ensure segmented/archive execution routes through `LocationRoute`, not a parallel path
  (feeds **VFS-001**). Severity: Medium.
- Scope a mutation-stable cursor design (joint CORE-008 / CORE-010 input). Severity: Medium.
