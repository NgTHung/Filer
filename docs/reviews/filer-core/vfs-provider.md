# VFS and Provider Abstraction Review (CORE-009)

Status note, 2026-06-24: this report is historical. The incomplete S3, WebDAV, FTP/SFTP,
FUSE, Kubernetes, and `RemoteProvider` stubs it describes were removed from `filer-core`.
`PROVIDER-002` now tracks provider registry and VFS contract stabilization before any
concrete remote or mount adapter is added.

Does the `FsProvider` trait surface, its capability model, and its paging strategies form a
foundation that can carry remote, archive, and timeout-bound access without forcing a core-wide
rewrite when the second provider ships? This report evaluates the trait, the native-vs-fallback
paging split, the capability and routing model, the timeout-context gap (PROVIDER-001), and
segmented/archive routing readiness (VFS-001). It produces the concrete trait-shape proposal that
the architecture-fit review (CORE-005, R2) deferred to CORE-009. It is review-only and changes no
production code.

Evidence is cited as `path:line` against the crate at the time of review. Test modules under
`src/tests/` are out of scope.

## The provider surface in one paragraph

`FsProvider` (`vfs/provider.rs:71`) is the single core filesystem contract: `Send + Sync`, async,
keyed entirely on `&Path`. It exposes `scheme`, `capabilities`, `paging`, read-side methods
(`list`, `list_with_options`, `list_page`, `read`, `read_range`, `exists`, `metadata`,
`read_header`, `open_reader`) and write-side methods (`write`, `copy`, `rename`, `delete`,
`mkdir`). The defaults are well chosen: `list_with_options` falls back to `list`
(`provider.rs:90`), `list_page` materializes and slices (`provider.rs:103`), `read_header`
delegates to `read_range` (`provider.rs:153`), `open_reader` fetches all bytes into a seekable
`Cursor` (`provider.rs:167`), and every mutation defaults to `Err(permission_denied)`
(`provider.rs:172-186`). Only `LocalFs` (`vfs/local.rs`) implements it for real. The trait models
read-only and capability-limited providers honestly. Its three structural gaps are no
deadline/cancellation context, a connection-lifecycle trait that the shared-ownership wiring
cannot call, and a path-only signature that cannot express layered/segmented routes.

## The trait surface is sound but has no deadline or cancellation context

This is the highest-severity abstraction gap and the one the architecture-fit review explicitly
handed to CORE-009 (`architecture-fit.md:68-79`).

Every `FsProvider` method takes `&self` and paths and nothing else (`provider.rs:71-187`). There
is no deadline, timeout budget, or cancellation token in any signature. `LocalFs` is cooperative
because tokio's filesystem calls yield, so a per-call `tokio::time::timeout` at the actor would
work for it. A remote provider does not get that for free: a stalled socket read inside a future
that holds no cancellation token keeps the spawned task alive after the actor's
`CancellationToken` (`actors/cancel.rs`) flips, because the token is checked between awaits in the
scanner (`paging.rs:93`, `:101`, `:114`) but never reaches the provider's I/O. The non-blocking
ambition breaks the moment a non-local provider serves a page.

The cancellation token already flows as far as `PagingSessions::load_provider`
(`paging.rs:63-71`, parameter `cancel: &CancellationToken`); it simply stops at the provider
boundary. Retrofitting a deadline parameter later is the widest possible change: the trait, every
provider impl, and every actor call site. That argues for landing it before the second provider.

### Concrete trait-shape proposal

Add one extensible context struct, passed by reference to the I/O methods, rather than a bare
`Duration`, so later additions (progress sink, byte budget) do not re-trigger a trait-wide change:

```rust
pub struct ProviderCx<'a> {
    pub deadline: Option<std::time::Instant>,
    pub cancel: Option<&'a CancelSignal>,
}
```

Three decisions matter more than the field list:

- **Layering.** A `CancellationToken` is an actor concept (`actors/cancel.rs`). Putting it directly
  on the `vfs` trait makes `vfs` depend on `actors`. Define a minimal provider-level cancel
  primitive in `vfs` or `model` (an `Arc<AtomicBool>` newtype, `CancelSignal`) and have the actor
  token deref/convert into it, so the dependency points downward, not into the actor layer.
- **Migration cost control.** Give the methods a defaulted context so existing call sites compile
  unchanged during the migration, for example `ProviderCx::none()`, then thread the real context
  through the actors that already hold a token (scanner, searcher, previewer, operator). This lets
  PROVIDER-001 land in reviewable stages instead of one wide diff.
- **Error contract.** A breached deadline must return `ErrorCode::TimedOut` (`errors.rs:62`), which
  already exists and maps to `ErrorKind::Timeout` (`errors.rs:91`), carrying provider context.
  PROVIDER-001 criterion three is therefore already expressible; only the plumbing is missing.

This is the concrete shape CORE-005 asked for. Implementation stays in PROVIDER-001.

## `read` loads whole files into memory; remote `open_reader` is a hidden full fetch

`read(&self, path) -> Vec<u8>` (`provider.rs:133`) has no size bound, and the default
`open_reader` (`provider.rs:167`) calls `read` and wraps the result in `Cursor<Vec<u8>>`. For
`LocalFs` this is fine because it overrides `open_reader` with a streaming `BufReader<File>`
(`local.rs:203`). For any remote provider that does not override it, opening a reader silently
downloads the entire object into memory. The media preview path already calls the unbounded form
(`services/preview/providers/media.rs:35` reads the whole file). Pair the deadline work with a
documented expectation that remote providers override `open_reader` and `read_header` rather than
inherit the buffering default, or the large-file case becomes a memory blowup the first time a
remote provider is wired in. Severity medium, realized only with a non-local provider.

## The capability model is split across three mechanisms and is too coarse for operations

Capability is currently expressed three different ways:

- `Capabilities { read, write, watch, search }` (`provider.rs:21-28`), four booleans.
- `ProviderPaging::{Fallback, Native}` as a separate method (`provider.rs:31-35`, `:78`).
- Per-method `Err(permission_denied)` defaults plus `read_header` returning `Err` to signal "no
  magic-byte support" (`provider.rs:153` doc, `:172-186`).

The booleans are too coarse for the write side. `capability.rs` derives every
`LocationOperationCapability` from the single `write` bool (`capability.rs:71-86`), so a provider
that can read and delete but not create cannot say so: copy, move, delete, rename, and mkdir all
collapse to one flag. The conflict and cross-provider policies are placeholder single-variant
enums (`LocationConflictPolicy::FailIfExists`, `LocationCrossProviderPolicy::Unsupported`,
`capability.rs:35-44`), hardcoded at construction (`capability.rs:81-82`). That is acceptable for a
local-only present, but PROVIDER-002 ("Provider connection management uses profile identifiers and
capability contracts") and operations across providers will need per-operation capability, not one
`write` bit. Recommendation: when PROVIDER-001 reshapes the trait, fold `paging` into the
capability report and replace the `write` bool with a per-operation capability set, so capability
lives in one place. Severity medium; it is a contract-shaping decision, not a present bug.

## Paging: two cursor namespaces that do not compose, so "native" still rewalks the directory

There are two paging strategies and, separately, two cursor schemes, and the interaction is the
real finding.

`LocalFs` reports `ProviderPaging::Native` (`local.rs:43`) and implements a true streaming
`list_page` that reads `read_dir` with an offset cursor and stops at the page limit
(`local.rs:86-158`). The default `list_page` (`provider.rs:103`) is offset-over-fresh-full-listing,
which is not stable under concurrent mutation (architecture-fit R4, `architecture-fit.md:90-96`).

The scanner does not consume the provider cursor directly. `PagingSessions::load_provider`
(`paging.rs:63`) runs its own keyset cursor (`paging:v1:`, `paging.rs:28`) over pipeline output,
and for a `Native` provider it loops calling `list_page` page-by-page until the provider reports
complete, feeding every entry into `PageSelection` (`paging.rs:98-125`). `PageSelection` keeps only
`limit + 1` sorted entries (`paging.rs:308`), so memory is bounded, but to serve one keyset page it
walks the entire directory through the provider. The provider's native cursor therefore avoids one
large allocation but not the full per-page directory walk: every "next page" re-lists the whole
directory. For the large-directory proof target this is O(directory size) work per page request,
not O(page).

The two cursors do not compose because the keyset reselection needs the full sorted order to find
"the next `limit` rows after the last row," while the provider cursor only knows raw directory
order. Closing this needs either a provider that can page in the pipeline's sort order (rare) or a
cached materialized order keyed by the keyset cursor so subsequent pages skip the rewalk. This sits
between CORE-008 (the non-cancellable `extend` loop it noted) and CORE-010 (pipeline/model). Flag
it as a scalability ceiling of the current abstraction, not a correctness bug. Severity medium for
the large-directory target.

## Provider resolution does not exist: the system is hardwired to one local provider

There is no provider registry and no scheme-to-provider resolution anywhere in the crate.
`FilerCore::with_defaults` constructs `Arc::new(LocalFs::new(...))` once and clones that single
`Arc` into the scan, search, preview, and operations modules (`api/handle.rs:144-158`). The
`ProviderRef` enum (`Local`, `Profile`, `Ephemeral`, `location.rs:11-20`) and `scheme()` exist in
the model, but nothing maps a `Location`'s scheme or `ProviderRef` to an `FsProvider`. Even if S3
or WebDAV were fully implemented, no code path could route a session to them. Unsupported routes
are correctly rejected (`location.rs:232`, `scanner.rs:213`), but there is no resolution layer to
ever produce a non-local provider. This is owned by PROVIDER-002, and it means every other provider
in `vfs/` is presently unreachable.

## The `RemoteProvider` lifecycle trait cannot be called through the shared-ownership wiring

This is a concrete design flaw that will force rework, not a future-shaped risk.

`RemoteProvider` (`vfs/remote.rs:32`) declares `connect(&mut self)`, `disconnect(&mut self)`, and
`ensure_connected(&mut self)`. But providers are shared everywhere as `Arc<dyn FsProvider>`
(`scan/mod.rs:38`, `preview/mod.rs:23`, `operations/mod.rs:31`, `search/mod.rs`), an immutable
shared reference. You cannot obtain `&mut self` from an `Arc<dyn FsProvider>` held by multiple
actors, so `connect`/`ensure_connected` are unreachable through the only wiring that exists.
Separately, the actors hold `Arc<dyn FsProvider>` and never see `RemoteProvider` at all, so even
the run loop has no hook to call `ensure_connected` before an operation.

The fix is structural and worth deciding before any remote provider is written: drop the
`&mut self` lifecycle trait and have remote providers self-connect lazily inside their `&self`
methods behind interior mutability (the `connected: bool` fields become atomics, the connection or
pool lives behind a `Mutex`/`RwLock`). The `connect`/`disconnect` API as written assumes exclusive
ownership the architecture does not provide. Severity high for PROVIDER-002, because the first SFTP
or S3 implementation written against the current `RemoteProvider` trait would not compile into the
actor wiring.

## Segmented and archive routing: the model is ready, the provider signature is not

The addressing model is in good shape. `LocationSegment::{ArchiveMember, Virtual}`
(`location.rs:22-27`), ordered `segments` on the descriptor (`location.rs:34`), `LocationRoute`
classification (`location.rs:54-68`), and `!/`-joined display (`location.rs:181-185`) together
describe nested archive and virtual paths cleanly. Execution is correctly deferred:
`require_direct_path()` rejects `Segmented` and `UnsupportedProvider` with structured errors
(`location.rs:223-243`), the scanner honors that rejection (`scanner.rs:210-222`), and `ArchiveFs`
is `#[allow(dead_code)]` returning `unsupported()` from every method (`vfs/archive.rs:9-61`). This
"represent first, execute later" order matches VFS-001 and is the right call.

The abstraction gap CORE-009 should name for VFS-001: `FsProvider` is keyed only on `&Path`
(`provider.rs:83`, `:133`), so a provider never receives the segment chain. Archive access needs
two things the path-only signature cannot express. First, member-relative addressing: the archive
provider must be told which member inside which archive, and today the only carrier is a `&Path`,
so `ArchiveFs::new(archive_path)` (`archive.rs:16`) bakes the archive in at construction and the
member would arrive as the path. That works for a single archive layer. Second, layering: an
archive inside an archive, or an archive served over a remote provider, has no composition
mechanism, because a provider cannot wrap another provider's byte stream through this trait
(`FuseFs` is the only provider that holds an inner `Box<dyn FsProvider>`, `vfs/fuse.rs:32`, and it
only forwards, it does not compose addresses). VFS-001 will need to decide how a `Segmented` route
maps to a provider instance plus a member path, and whether nesting requires a provider-stacking
abstraction. The model supports describing the nesting; the provider trait does not yet support
executing it. Severity medium, realized only when archive execution lands.

## PROVIDER-ACCESS adherence: previews bypass the provider, metadata does not

The previewer is split, and the split is the finding. Metadata extraction is provider-correct: the
previewer reads the magic-byte header through the provider (`previewer.rs:464`, `:544`,
`provider.read_header(&path, 512)`) and hands the provider to the extractor
(`previewer.rs:481`, `:558`, `.extract(&path, &mime, provider.as_ref())`). Preview rendering is
not: it calls `preview_registry.generate_with_options(&path, &opts)` with only the path
(`previewer.rs:162`, `:254`), and the preview providers then open local files directly, bypassing
`FsProvider` entirely:

- `services/preview/providers/archive.rs:22`, `:52` — `std::fs::File::open`
- `services/preview/providers/code.rs:89` — `tokio::fs::File::open`
- `services/preview/providers/media.rs:35` — `tokio::fs::read`
- `services/preview/providers/text.rs:30` — `tokio::fs::File::open`
- `services/preview/registry.rs:120` — `tokio::fs::File::open`

`PreviewProvider::generate` takes a `&Path` (`services/preview/provider.rs:149-151`), so the
provider is structurally unavailable to renderers. Previews are therefore local-only and silently
violate PROVIDER-ACCESS for any non-local provider, even though the trait already exposes the right
tools (`open_reader` returns a seekable `Box<dyn ReadSeek>`, and `read_header` is provider-backed).
This is the largest live PROVIDER-ACCESS gap and is owned by PREVIEW-001 ("Magic-byte reads and
cache generation use provider-backed access"). Severity medium now (local-only build), high the
moment a remote provider is reachable.

A lower-severity note on the model constructors: `FileNode::from_path` canonicalizes through
`std::fs::canonicalize` (`model/node.rs:86`, `:91`) and stats with `std::fs::metadata`
(`node.rs:96`), and `LocalFs::metadata` delegates to it (`local.rs:196`). These are the local
building blocks, which is acceptable, but `from_path`'s local-only canonicalization must not sit on
any non-local provider's path. Providers should build `FileNode`s from their own metadata
(`FileNode::from_metadata`), not route through `from_path`. Note it for PREVIEW-001/PROVIDER-002;
not a present bug.

## Unimplemented providers panic instead of degrading

Superseded, 2026-06-24: the incomplete feature-gated providers were deleted instead of
rewritten as unsupported stubs. This section remains as audit evidence for why the old surface
was removed.

`S3Fs`, `WebDavFs`, `FtpFs`, and `K8sFs` implement every `FsProvider` and `RemoteProvider` method
as `todo!()` (`vfs/s3.rs:83-118`, `vfs/webdav.rs:94-129`, `vfs/ftp.rs:102-137`,
`vfs/kubernetes.rs:139-174`), and `FuseFs::mount`/`unmount` are `todo!()` (`vfs/fuse.rs:45-52`).
These are feature-gated out of the default build (`vfs/mod.rs:7-30`), so they do not compile today.
But if a feature flag is enabled, the provider compiles and panics at runtime on first call rather
than returning a structured error. Contrast `ArchiveFs`, which correctly returns
`CoreError::unsupported_operation` from every method (`archive.rs:20-61`). `unsupported_operation`
already exists (`errors.rs:71`, `ErrorCode::UnsupportedOperation`), so the safe pattern is
available. Recommendation: replace the `todo!()` bodies with `Err(unsupported_operation(...))` so an
enabled-but-incomplete provider degrades instead of panicking, and so it cannot become a panic
surface once a resolution layer (PROVIDER-002) can reach it. Severity low while feature-gated and
unreachable, but it is a latent panic surface and conflicts with the AGENTS no-panic-in-production
rule the moment a feature is turned on.

A related minor note: the provider config structs embed secrets in plaintext fields
(`RemoteConfig.password` `remote.rs:14`; `S3Config.secret_key`/`access_key` `s3.rs:15-16`;
`FtpConfig.password`/`private_key` `ftp.rs:14-18`; `WebDavConfig.password`/`bearer_token`
`webdav.rs:14-15`). PROVIDER-002 already owns this ("serializable without portable secrets"); flag
only so the secret-handling decision is made when those configs become serializable.

## Summary table

| Axis | State | Severity |
| --- | --- | --- |
| Trait surface and defaults | Sound; honest capability-limited defaults | — |
| Deadline/cancellation context | Absent from every method; token stops at provider boundary | High |
| Unbounded `read` / default `open_reader` | Full in-memory fetch for non-overriding remote providers | Medium |
| Capability model | Split across three mechanisms; write side too coarse | Medium |
| Paging | Native cursor and keyset cursor do not compose; full rewalk per page | Medium |
| Provider resolution | No registry; hardwired to one `LocalFs` | High (PROVIDER-002) |
| `RemoteProvider` lifecycle | `&mut self` unreachable through `Arc<dyn FsProvider>` wiring | High (PROVIDER-002) |
| Segmented/archive routing | Model ready; path-only trait cannot express layering | Medium (VFS-001) |
| PROVIDER-ACCESS in previews | Renderers bypass the provider and open local paths | Medium→High (PREVIEW-001) |
| Unimplemented providers | Superseded. Incomplete feature-gated providers were removed on 2026-06-24. | Closed |

## Follow-up task candidates

Candidates for the CORE-013 remediation backlog, not new tasks created here. Each names the task
that already owns the outcome where one exists.

- Land the `ProviderCx` deadline/cancellation context on `FsProvider` before the second provider
  ships, using a `vfs`/`model`-owned cancel primitive to avoid a `vfs`→`actors` dependency and a
  defaulted context to stage the migration. Feeds PROVIDER-001. Severity: High.
- Redesign `RemoteProvider` for shared ownership: drop `&mut self`, self-connect lazily behind
  interior mutability so the lifecycle is reachable through `Arc<dyn FsProvider>`. Feeds
  PROVIDER-002. Severity: High.
- Build the provider resolution layer (scheme/`ProviderRef` to `Arc<dyn FsProvider>`) so non-local
  providers are reachable at all. Feeds PROVIDER-002. Severity: High, but gated behind the two
  above.
- Route preview rendering through the provider: give `PreviewProvider::generate` provider-backed
  access (`open_reader`/`read_header`) instead of a bare `&Path`. Feeds PREVIEW-001. Severity:
  Medium now, High once a remote provider is reachable.
- Define how `Segmented` routes map to a provider instance plus member path, and decide whether
  nested layers need a provider-stacking abstraction, before archive execution is written. Feeds
  VFS-001. Severity: Medium.
- Replace the coarse `write` bool with a per-operation capability set and fold `paging` into the
  capability report, so capability lives in one place. Feeds PROVIDER-002. Severity: Medium.
- Resolve the keyset-vs-provider cursor non-composition so a "next page" does not rewalk the whole
  directory (cached materialized order, or pipeline-ordered provider paging). Joint with CORE-008
  and CORE-010. Severity: Medium for the large-directory target.
- Incomplete feature-gated remote and mount providers were removed on 2026-06-24. Future
  providers should start from the stabilized registry and capability contracts.
- Document the expectation that remote providers override `open_reader`/`read_header` rather than
  inherit the full-buffering default. Pairs with PROVIDER-001. Severity: Low.
