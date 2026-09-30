# Comparative Performance Benchmark Design

## Purpose

This suite tells you whether Filer-core is becoming faster, whether it is
competitive with relevant libraries and file-manager frameworks, and where
time is spent across a complete user action.

Keep three result sets separate:

1. Engine results compare directory and search implementations in process.
2. Core results measure Filer through public commands and events.
3. Application results measure input to a correct visible frame.

Do not combine these layers into one score. Process startup, terminal drawing,
provider work, pipeline work, and output formatting answer different questions.

## Scope

The first implementation stays under Filer-core. It may include a reference
client that consumes public core events and commits a deterministic virtual
view. It must not require changes to filer-app.

External application adapters run released binaries in isolated environments.
They are benchmark tools, not Filer-core dependencies. Full filer-app coverage
can use the same protocol when the full application work resumes. The separate
app:UI-011 validation track supplies real-window feedback during 0.3.1 and does
not become a dependency of this benchmark package.

The full program covers the scenarios below. The 0.3.1 slice implements only
flat-10k/flat-100k, fast and metadata browsing, continuation, and one journey
through paging, name sorting, filtering, and refresh. CORE-029 owns the full
program; its completion is outside the 0.3.1 exit gate. CORE-042 owns the
remaining fixtures, journeys, and internal trace attribution.

The full suite covers:

- flat directory browsing and continuation
- metadata enrichment
- sorting, filtering, and grouping
- recursive search, first match, completion, and cancellation
- navigation sequences, refresh, and cache reuse
- filesystem mutation convergence
- semantic decorations through the completed MODULES-002 contract
- input responsiveness while long work is active

## Benchmark Layers

| Layer | Filer path | Comparison set | Boundary |
|---|---|---|---|
| Engine | Local provider and pipeline functions | `std::fs`, Tokio, GIO, KIO | Adapter call to canonical rows |
| Recursive search | Search provider and matcher | `walkdir`, `jwalk`, `ignore::WalkParallel` | Search request to canonical matches |
| Core | `Command` to terminal `Event` | Filer revisions | Public command and event contract |
| Reference application | Semantic input to virtual-view commit | Filer revisions | Input injection to correct committed view |
| External application | Process or steady-state input to visible frame | Yazi and Broot | Black-box input to correct terminal frame |

GIO and KIO are framework comparisons, not whole-application comparisons.
Yazi and Broot are whole applications, so their results never share an
in-process leaderboard with Filer-core.

## Initial Competitors

### Flat Directory Engines

- `std::fs::read_dir` provides the synchronous Rust lower bound.
- `tokio::fs::read_dir` shows the async runtime cost Filer already builds on.
- GIO `GFileEnumerator` provides synchronous and batched asynchronous listing
  with selectable metadata attributes.
- KDE `KCoreDirLister` provides incremental items, completion, cancellation,
  filtering, and update signals without a graphical view.

### Recursive Search Engines

- `walkdir` provides a simple sequential traversal reference.
- `jwalk` provides parallel traversal for multi-directory trees.
- `ignore::WalkParallel` provides parallel traversal with hidden, glob, and
  ignore-file behavior.

Do not use recursive walkers as flat-directory competitors. Parallel walkers
solve a different problem when only one directory is read.

### Whole Applications

- Yazi is the primary speed-focused Rust file-manager comparison.
- Broot is the primary tree-navigation and interactive-search comparison.

Pin every application by release and binary digest. Record its configuration
with each run. If an application cannot express a scenario with matching
semantics, report `not_supported`. Do not give it a zero or estimate the result.

## Versioned Protocol

Every adapter reads one scenario request and writes newline-delimited JSON
events. A protocol version prevents an old adapter from silently producing an
incompatible result.

Version 1 uses one UTF-8 JSON object followed by `\n` on standard input. The
adapter writes only UTF-8 JSON objects followed by `\n` on standard output.
Diagnostics go to standard error. Objects are strict: a receiver rejects
unknown fields, missing fields, duplicate object keys, non-integer JSON
numbers, and values outside the ranges below. A schema change that alters a
required field or its meaning requires a new protocol version.

### Request Schema v1

The request is one object with these required fields:

| Field | Type and rule |
|---|---|
| `protocol_version` | Integer `1`. |
| `type` | String `run_request`. |
| `run_id` | Opaque identifier shared by a benchmark run. |
| `sample_id` | Opaque identifier unique within `run_id`. |
| `process_id` | Opaque identifier for the adapter process. |
| `order_id` | Opaque identifier for the randomized round and position. |
| `scenario_id` | A scenario identifier declared in this document. |
| `fixture` | Object containing exact `id` and `digest` strings from a fixture manifest. |
| `implementation` | Object containing `id`, `version`, `source_revision`, `build_profile`, and executable `binary_digest`. |
| `adapter` | Object containing `id`, `version`, and `binary_digest`. |
| `environment` | Object containing machine and filesystem profile identifiers and their digests. |
| `cache` | Object containing `process`, `filesystem`, and `semantic` states. |
| `viewport_size` | Integer from 1 through `page_size`. Version 1 uses 40. |
| `page_size` | Integer from 1 through 4096. Version 1 uses 256. |
| `requested_fields` | Non-empty ordered set drawn from the canonical row fields. |
| `sort` | Object with `field` and `direction`. |
| `filter` | A tagged filter object. |
| `group` | A tagged group object. |
| `search` | A tagged search object. |
| `clock` | Exact object `{"kind":"process_monotonic","unit":"nanosecond"}`. |

Opaque identifiers match `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`. Digests match
`sha256:[0-9a-f]{64}`. The runner rejects a repeated `(run_id, sample_id)` and
an event whose four correlation identifiers do not equal its request.

`requested_fields` uses this canonical order: `identity`, `kind`,
`size_bytes`, then `modified_unix_ns`. `identity` and `kind` are always
required. A metadata browse adds `size_bytes` and `modified_unix_ns`. An
adapter must not collect extra metadata during the timed interval and must not
emit unrequested row fields.

Version 1 accepts `provider_order` with direction `none`, or `name` with
direction `ascending`. Its filter is either `{"kind":"none"}` or
`{"kind":"name_contains","value":"file-0001","case_sensitive":true}`.
Its `group` and `search` objects are both `{"kind":"none"}`. Later scenarios
may add tagged variants only in a new protocol version.

Cache values are closed enums. `process` is `cold` or `warm`. `filesystem` is
`controlled_cold`, `fresh_copy`, `warm`, or `uncontrolled`. `semantic` is
`empty`, `reset`, or `reused`. Samples with different cache triples do not
share a result set.

A valid request is:

```json
{"protocol_version":1,"type":"run_request","run_id":"run-local-001","sample_id":"sample-0001","process_id":"process-0001","order_id":"round-01-position-02","scenario_id":"browse.fast.first","fixture":{"id":"flat-10k-v1","digest":"sha256:b684d98507db303ffc02805732bfc721d65af093db142b32887c35b0d0e5a95e"},"implementation":{"id":"filer-core","version":"0.3.1","source_revision":"0123456789abcdef0123456789abcdef01234567","build_profile":"release","binary_digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000"},"adapter":{"id":"filer-public","version":"1.0.0","binary_digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111"},"environment":{"machine_profile_id":"linux-x86_64-lab-01","machine_profile_digest":"sha256:2222222222222222222222222222222222222222222222222222222222222222","filesystem_profile_id":"ext4-lab-01","filesystem_profile_digest":"sha256:3333333333333333333333333333333333333333333333333333333333333333"},"cache":{"process":"cold","filesystem":"warm","semantic":"empty"},"viewport_size":40,"page_size":256,"requested_fields":["identity","kind"],"sort":{"field":"provider_order","direction":"none"},"filter":{"kind":"none"},"group":{"kind":"none"},"search":{"kind":"none"},"clock":{"kind":"process_monotonic","unit":"nanosecond"}}
```

This request is invalid for at least two independent reasons: version 1 does
not permit a numeric string or an unknown field. Conformance tests mutate one
field at a time, so rejection-code precedence is not part of the protocol:

```json
{"protocol_version":"1","type":"run_request","extra":true}
```

The version rule applies after JSON type validation. A JSON integer other than
`1` returns `unsupported_protocol_version`; a JSON string, floating-point
number, or other non-integer value returns `invalid_schema`. The unknown
`extra` field in the example also returns `invalid_schema`. These are separate
single-mutation cases in the validator suite, so Serde's diagnostic wording is
never a public result contract.

The runner supplies two trusted inputs that are deliberately outside the wire
request: the adapter capability declaration and the names of requested
resource metrics. Capabilities identify supported scenario ids and whether the
adapter exposes streaming unfiltered listing with an observable examined-row
count. Requested metric names identify the metrics that must be reported on
`sample.completed`. Profile collection, profile persistence, cache preparation,
and real ready barriers remain runner responsibilities. The validator checks
only the declared references and the event fields defined here.

### Event Schema v1

Every event contains every field in this table. Use an empty object or array,
or `null` only where the table permits it. This keeps missing observations
distinct from unavailable observations.

| Field | Type and rule |
|---|---|
| `protocol_version` | Integer `1`. |
| `type` | String `run_event`. |
| `run_id`, `sample_id`, `process_id`, `order_id` | Exact request values. |
| `sequence` | Unsigned 64-bit integer, starting at zero and increasing by exactly one. |
| `timestamp_ns` | Unsigned 64-bit nanoseconds from the adapter process monotonic clock. |
| `phase` | One of the phase names below. |
| `action_id` | A scenario action identifier, or `null` for sample phases. |
| `counts` | Object with `examined`, `accepted`, `emitted`, and `visible`. |
| `rows` | Array of canonical rows. Empty when the phase exposes no rows. |
| `output` | A semantic output object, or `null`. |
| `metrics` | Object from metric name to an observed or unavailable value. |
| `status` | A terminal status object on `sample.completed`, otherwise `null`. |

The phase enum is `sample.started`, `action.started`, `row.first`,
`viewport.committed`, `page.committed`, `listing.completed`,
`transform.completed`, `view.committed`, `action.completed`, and
`sample.completed`. Scenario tables below select the required subset and
order. A success trace has exactly one `sample.started` first and one
`sample.completed` last. It has one `action.started` and one
`action.completed` for each prescribed action.

Counts are cumulative within an action and never decrease. Each count is an
unsigned 64-bit integer or an unavailable value. When each relevant pair is
observed, `accepted <= examined`, `emitted <= accepted`, and
`visible <= emitted`.
`action.started` resets action counts to zero. Sample phases use totals across
completed actions. Correctness-required counts cannot be unavailable. An
unavailable value has this exact shape:

```json
{"unavailable":"not_observable"}
```

The reason is `unsupported`, `permission_denied`, `not_observable`, or
`platform_unavailable`. Metrics use the same value shape. Adapters must not
write zero when a count or metric is unavailable. `emitted` and `visible` are
required on commit phases. `accepted` is also required for a filter.
`examined` is required only when the adapter declares an observable streaming
work metric; otherwise its structural gate is `not_evaluable`.

Canonical rows contain exactly the requested fields. `identity` is a non-empty
UTF-8 fixture-relative path using `/`; it cannot start with `/`, contain `.` or
`..` segments, contain `\\`, or contain NUL. `kind` is `file` or `directory`.
`size_bytes` is an unsigned 64-bit integer for a file and `null` for a
directory. `modified_unix_ns` is a signed 64-bit integer. The initial flat
fixtures use ASCII identities. CORE-042 must version the identity encoding
before adding non-UTF-8 names.

`rows` contains one row for `row.first`; the complete committed viewport or
page for its commit phase; every observed row for `listing.completed`; and the
complete transformed result for `transform.completed`. `view.committed`
contains its visible viewport. Other phases use an empty array. Rows within an
event are unique. A provider-order continuation chain cannot repeat an
identity across page commits. `output` has `scope`, `digest`, `row_count`, and
`continuation`. Scope is `membership`, `metadata`, `ordered`, `page`, or
`viewport`. Continuation is `more`, `end`, or `not_applicable`. Every phase
that carries rows requires an output and matching row count; other phases
require `null`.

Terminal status is one of these exact objects:

```json
{"kind":"success","code":null,"message":null}
{"kind":"not_supported","code":"scenario_not_supported","message":"browse.refresh is not supported"}
{"kind":"error","code":"fixture_digest_mismatch","message":"fixture content does not match its manifest"}
{"kind":"cancelled","code":"cancelled_at_barrier","message":"the scenario reached its cancellation barrier"}
```

Only `success` is eligible for timing. `not_supported` is a capability result,
not a zero-duration sample. `error` and `cancelled` preserve diagnostics but
are excluded from rankings. A non-success trace may go directly from
`sample.started` to `sample.completed`; a success trace must contain every
required scenario phase.

A complete valid adapter-support result is:

```json
{"protocol_version":1,"type":"run_event","run_id":"run-local-001","sample_id":"sample-0001","process_id":"process-0001","order_id":"round-01-position-02","sequence":0,"timestamp_ns":4100,"phase":"sample.started","action_id":null,"counts":{"examined":0,"accepted":0,"emitted":0,"visible":0},"rows":[],"output":null,"metrics":{},"status":null}
{"protocol_version":1,"type":"run_event","run_id":"run-local-001","sample_id":"sample-0001","process_id":"process-0001","order_id":"round-01-position-02","sequence":1,"timestamp_ns":4200,"phase":"sample.completed","action_id":null,"counts":{"examined":0,"accepted":0,"emitted":0,"visible":0},"rows":[],"output":null,"metrics":{"cpu_time_ns":{"unavailable":"not_observable"}},"status":{"kind":"not_supported","code":"scenario_not_supported","message":"browse.fast.first is not supported"}}
```

This event is invalid because its row array has one row while
`output.row_count` is 256. Its digest is the correct page digest for the one
shown row, which isolates the count failure:

```json
{"protocol_version":1,"type":"run_event","run_id":"run-local-001","sample_id":"sample-0001","process_id":"process-0001","order_id":"round-01-position-02","sequence":3,"timestamp_ns":8900,"phase":"page.committed","action_id":"open","counts":{"examined":256,"accepted":256,"emitted":256,"visible":40},"rows":[{"identity":".dir-000000","kind":"directory"}],"output":{"scope":"page","digest":"sha256:b01ec7b38aa3ead7298b439da888bd9e943cb15bd431d05d3dee13698bb4c2f9","row_count":256,"continuation":"more"},"metrics":{},"status":null}
```

A trace validator rejects it with `output_row_count_mismatch`. CORE-039 golden
traces must contain every row claimed by an output.

Event framing and lexical errors have stable ownership. A request must contain
one UTF-8 JSON object and exactly one terminating newline. Event input is a
non-empty sequence of UTF-8 lines, each with one JSON object and one
terminating newline. Truncation, invalid UTF-8, and invalid JSON syntax return
`malformed_json`. An otherwise valid JSON object on adapter stdout whose
`type` is not `run_event`, and a plain-text stdout diagnostic, return
`unexpected_stdout`. An unknown phase returns `invalid_phase`; a row with an
unsafe identity, wrong kind, invalid metadata, or an incorrect requested-field
projection returns `invalid_row`.

### Clock and Digest Rules

All timestamps in one sample come from the same process-local monotonic clock.
They are nondecreasing by `sequence`. They are not Unix time and cannot be
compared across processes. The adapter samples the clock for `action.started`
immediately before invoking the measured action. Runner wall-clock timestamps
belong in raw-result metadata, not event timestamps.

Every digest uses SHA-256 and the `sha256:` prefix. Canonical digest input is a
sequence of UTF-8 tokens. Encode a token as its byte length in ASCII, `:`, then
its bytes. Concatenate these tokens without separators:

1. `filer-benchmark-digest-v1`
2. the scope
3. the decimal field count, followed by each field name
4. the decimal row count
5. each row value in field order, using `~` for JSON `null` and base-10
   integers without leading zeroes

Membership and metadata inputs sort rows by identity bytes. Membership hashes
only `identity`. Metadata hashes `identity`, `kind`, `size_bytes`, and
`modified_unix_ns`. Ordered, page, and viewport inputs preserve the committed
row order and hash exactly the requested fields. This encoding makes provider
enumeration order irrelevant to membership while preserving order where a
scenario requires it.

## Correctness Before Timing

Every timed sample must prove semantic equivalence.

The fixture manifest records expected digests for:

- complete row membership, independent of unspecified provider enumeration order
- sorted rows
- grouped labels and group order
- filtered rows
- search matches
- visible viewport state after each scripted action

An adapter result is invalid when its digest, row count, completion state, or
error behavior differs. Invalid results never appear in a performance ranking.

Use fixture-relative identities for membership digests. Check order only when
the scenario requests it. For provider-order pages, validate visible rows against
the adapter's observed sequence and validate uniqueness and membership across
the completed chain. Different filesystem enumeration orders remain valid.

For mutable scenarios, record the expected final generation and digest. Measure
convergence only after correctness identifies the authoritative generation.

## Fixture Corpus

Generate fixtures from versioned manifests so every adapter sees the same
names, types, metadata, permissions, and mutations.

| Fixture | Purpose |
|---|---|
| `flat-10k` | System32-scale first paint and paging |
| `flat-100k` | Scaling and accidental full-walk detection |
| `tree-100k` | Recursive traversal and search |
| `sparse-match-100k` | Filters and searches whose matches appear late |
| `git-10k` | Clean, modified, added, deleted, ignored, untracked, and conflicted rows |
| `hostile-10k` | Unicode, non-UTF-8 Unix names, symlinks, long names, and permission errors |
| `mutation-10k` | Create, delete, rename, and metadata changes during an active view |

Use a realistic mix of files and directories. Vary extensions, sparse sizes,
timestamps, hidden state, and permissions. Fixture creation is never part of a
timed sample.

Record the filesystem type and mount options. Keep tmpfs, Btrfs, ext4, APFS,
and NTFS results separate.

### Flat Manifest Schema v1

The initial manifest object contains `schema_version`, `id`, `generator`,
`entry_count`, `requested_metadata`, `expected`, and `manifest_digest`.
`generator` contains every generation parameter. `expected` contains
membership, metadata, and name-order digests plus applicable
reference-journey digests. Unknown or missing fields are invalid.

`flat-v1` generates index `i` in `[0, entry_count)`:

1. The entry is a directory when `i % 10 == 0`; otherwise it is a file.
2. Prefix the name with `.` when `i % 25 == 0`.
3. A directory is `dir-{i:06}`. A file is `file-{i:06}.{extension}`.
4. File extensions are `["rs","txt","log","bin"]`, selected by `i % 4`.
5. File size is `((i * 7919) % 1048573) + 1`. Creation may use sparse files.
6. Modification time is `1704067200000000000 + i * 1000000000` Unix
   nanoseconds for both files and directories.

Create all entries before applying final modification times so directory
creation cannot change a recorded timestamp. The root path, inode, allocation,
creation time, access time, owner, and provider enumeration position are not
fixture identity. Directories have `size_bytes: null` in canonical rows.

The manifest digest uses the digest algorithm above with scope `manifest` and
fields `name`, `value`. Hash these ordered records: `manifest_id`,
`entry_count`, `schema_version`, `generator`, `directory_every`,
`hidden_every`, `extensions`, `size_multiplier`, `size_modulus`, `size_offset`,
`modified_base_unix_ns`, `modified_step_ns`, `membership_digest`,
`metadata_digest`, and `name_order_digest`. Values are the exact strings shown
by the manifest, with extensions encoded as `rs,txt,log,bin`. The
`manifest_digest` field itself is excluded.

The normative manifests are:

```json
{"schema_version":1,"id":"flat-10k-v1","generator":{"id":"flat-v1","directory_every":10,"hidden_every":25,"extensions":["rs","txt","log","bin"],"size_multiplier":7919,"size_modulus":1048573,"size_offset":1,"modified_base_unix_ns":1704067200000000000,"modified_step_ns":1000000000},"entry_count":10000,"requested_metadata":["size_bytes","modified_unix_ns"],"expected":{"membership_digest":"sha256:ac84c83acce8b289874fc468a4aa77b113404ff041e54af0ba588cc1e2c905a9","metadata_digest":"sha256:581febb0f3f93343ada6474ed2d99d706c0a03494c303267968e158454a2ef81","name_order_digest":"sha256:a83cb70360ea0d060fde788c3a5c0ab30fa2be56c0c4ac2bb604f78b8aa8ac22","name_viewport_digest":"sha256:f8637ffa842a13652e47c6523d87838eef4849433896fd37c9e746b42163ce3a","filter_count":90,"filter_order_digest":"sha256:193ef6adbff68a4b7fad2050ddb8b85e3c830484bbe3bcc4e557b86cb43161ea","filter_viewport_digest":"sha256:76d10051562590383e451a7965ff09b6af26a0e832ac329fa478aacc3663fc92"},"manifest_digest":"sha256:b684d98507db303ffc02805732bfc721d65af093db142b32887c35b0d0e5a95e"}
```

```json
{"schema_version":1,"id":"flat-100k-v1","generator":{"id":"flat-v1","directory_every":10,"hidden_every":25,"extensions":["rs","txt","log","bin"],"size_multiplier":7919,"size_modulus":1048573,"size_offset":1,"modified_base_unix_ns":1704067200000000000,"modified_step_ns":1000000000},"entry_count":100000,"requested_metadata":["size_bytes","modified_unix_ns"],"expected":{"membership_digest":"sha256:b32e48f7e2c31b06e7c5256e624b61c7f3d3bcbc4ede922a145ef7a4d544f1d3","metadata_digest":"sha256:d2dd00efed47da4987dc30ae8bd558cfad45d9aa9ce34d2b0f2f5ccb2349831d","name_order_digest":"sha256:119be04466ebb4673c185af2f177a771155a957f53f70fd7e30dec7201573dcf","name_viewport_digest":"sha256:f8637ffa842a13652e47c6523d87838eef4849433896fd37c9e746b42163ce3a"},"manifest_digest":"sha256:dd55706f9be421dafc15a7237493158c3703ea9a0e379e7d51f6653020e1566e"}
```

The expected name order compares UTF-8 identity bytes in ascending order.
`filter_*` selects identities containing `file-0001`, case-sensitively, after
name sorting. Provider-order pages have no golden page digest. Validate each
observed page digest against its emitted rows, reject duplicate identities,
and compare the completed chain's order-independent membership digest with the
manifest. This permits different valid filesystem enumeration orders.

## Scenario Contracts

Each scenario defines its start barrier, timed milestones, terminal condition,
correctness digest, and supported benchmark layers.

### Version 1 Barriers

Before `sample.started`, the runner validates the fixture manifest and profile
digests, establishes the requested cache state, and lets the adapter finish
untimed initialization. Before each `action.started`, the adapter has applied
all prior actions and has no pending work for them. The timestamp on
`action.started` is the timing origin. Setup work required by the action itself
cannot move before that event.

An output milestone becomes visible only when the consumer could use it. Reading
40 entries without committing them is not a viewport milestone. A page commit
contains that page's complete canonical rows. `listing.completed` proves the
full membership or metadata digest and carries all observed rows.
`transform.completed` proves and carries the complete ordered or filtered
result. `view.committed` proves the visible viewport. `action.completed`
follows all required outputs and pending work for that action.
`sample.completed` follows the final action. The event timestamp marks the
semantic milestone after canonical rows exist; NDJSON serialization after that
timestamp is not presented as provider or transform latency.

Version 1 supports these standalone scenarios:

| Scenario | Fixture and request | Start barrier | Required success milestones | Completion |
|---|---|---|---|---|
| `browse.fast.first` | `flat-10k-v1`; identity and kind; provider order | Fixture ready; no listing work started | `row.first`, 40-row `viewport.committed`, 256-row `page.committed`, `listing.completed` | 10,000 unique identities and the membership digest match |
| `browse.fast.scale` | `flat-100k-v1`; identity and kind; provider order | Fixture ready; no listing work started | 40-row viewport, 256-row page, `listing.completed` | 100,000 unique identities and the membership digest match |
| `browse.metadata.first` | `flat-10k-v1`; all four row fields; provider order | Fixture ready; no metadata work started | 40-row viewport, 256-row page, `listing.completed` | Membership and metadata digests match |
| `browse.next` | `flat-10k-v1`; identity and kind; provider order | Fixture ready; no listing work started | `open` commits page 1, then every page from 2 through 40 commits; pages 2, 10, and 40 are continuation milestones | Page 40 has 16 rows and `end`; the 40-page chain has no duplicates and matches membership |
| `view.sort.name` | `flat-10k-v1`; identity and kind; ascending name | Complete unsorted snapshot is ready | `transform.completed`, then `view.committed` | Full order and first-40 viewport digests match |
| `view.filter.common` | `flat-10k-v1`; identity and kind; ascending name; name contains `file-0001` | Complete name-sorted snapshot is ready | `transform.completed`, then `view.committed` | Exactly 90 rows and both filter digests match |
| `browse.refresh` | `flat-10k-v1`; identity and kind; ascending name | Warm complete snapshot is visible; no refresh work started | New `listing.completed`, `transform.completed`, then `view.committed` | A new enumeration matches membership; sorted output and viewport digests match |

Each row in the table expands to `action.started`, the listed milestones, and
`action.completed`. `browse.next` uses action ids `page-0002` through
`page-0040`. Other action ids are `open`, `sort-name`, `filter-name`, and
`clear-filter`, and `refresh`. A provider may emit viewport before page, but
each named milestone appears exactly once. `row.first` precedes every other
browse output when it is required.

The exact action and request interpretation is:

| Scenario | Initial request settings | Action order | Allowed optional phases and output scopes |
|---|---|---|---|
| `browse.fast.first` | `flat-10k-v1`, `identity,kind`, provider order, no filter, cold/controlled cache triple supplied by the runner | `open` | `row.first` is required; viewport, page, and listing outputs use `viewport`, `page`, and `membership` scopes |
| `browse.fast.scale` | `flat-100k-v1`, `identity,kind`, provider order, no filter, runner-declared cache triple | `open` | `row.first` is optional; viewport, page, and listing outputs use `viewport`, `page`, and `membership` scopes |
| `browse.metadata.first` | `flat-10k-v1`, all four fields, provider order, no filter, runner-declared cache triple | `open` | `row.first` is optional; viewport, page, and listing outputs use `viewport`, `page`, and `metadata` scopes |
| `browse.next` | `flat-10k-v1`, `identity,kind`, provider order, no filter, runner-declared cache triple | `open`, then `page-0002` through `page-0040` | `open` must commit page 1 before page 2 starts. Each page action commits one `page` output; page 40 also commits the final `membership` listing proof. `row.first` is optional only on `open`. |
| `view.sort.name` | `flat-10k-v1`, `identity,kind`, ascending name, no filter, warm semantic snapshot | `sort-name` | `transform.completed` uses `ordered`; `view.committed` uses `viewport` |
| `view.filter.common` | `flat-10k-v1`, `identity,kind`, ascending name, `name_contains` filter, warm semantic snapshot | `filter-name` | `transform.completed` uses `ordered`; `view.committed` uses `viewport` |
| `browse.refresh` | `flat-10k-v1`, `identity,kind`, ascending name, no filter, warm semantic snapshot | `refresh` | A new `listing.completed` uses `membership`, followed by `transform.completed` with `ordered` and `view.committed` with `viewport` |

The request fields do not change during a sample. Action ids carry the
scenario's semantic changes: `sort-name` selects ascending name order,
`filter-name` selects the case-sensitive `file-0001` filter, and
`clear-filter` restores the unfiltered ascending name view. The reference
journey starts with `flat-10k-v1`, `identity,kind`, provider order, no filter,
and a warm process with an empty semantic cache. Its actions are exactly
`open`, `page-0002` through `page-0040`, `sort-name`, `filter-name`,
`clear-filter`, and `refresh`; the same action-specific changes apply without
adding wire fields. A successful action has one `action.started` and one
`action.completed`, with its listed milestones between them. No phase is
optional when it is named as a required milestone.

Counts reset to zero at every `action.started`. Within an action they are
cumulative and describe new work, not rows repeated in a proof. Sample phases
report totals across completed actions. For example, in `browse.next`,
`page-0040` has 16 page rows, `continuation: "end"`, and action counts
`examined=16`, `accepted=16`, `emitted=16`, `visible=16`. Its
`listing.completed` event repeats all 10,000 chain rows with
`scope="membership"` and `row_count=10000`, but its counts remain 16 because
the proof does not emit a second copy of those rows. The final sample totals
are `examined=10000`, `accepted=10000`, `emitted=10000`, and `visible=10000`
when every page reports those four counts. The page 40 proof is therefore both
complete and non-inflating.

Provider enumeration order is observed, not prescribed. For provider-order
scenarios, page and viewport digests must match their event rows but do not
have golden values. The validator accumulates page identities and checks the
final order-independent membership digest. A continuation of `end` before the
manifest count or `more` at the final page is invalid.

Name sorting compares identity UTF-8 bytes in ascending order. It is a
snapshot-only transform in version 1. The sort clock starts only after the
complete input snapshot is ready, and the adapter cannot describe listing time
as sorting time. The filter uses the same complete snapshot. Sparse streaming
filters in the extended suite keep their own listing and filter costs and do
not inherit the unfiltered first-page examined-row gate.

### Initial Reference Journey

`journey.browse-reference` uses `flat-10k-v1` in one warm process. It performs
these actions without resetting semantic state:

1. `open` commits the observed first viewport and page, then pauses at its
   continuation barrier.
2. `page-0002` through `page-0040` continue the provider-order page chain.
   Pages 2, 10, and 40 are visible timing milestones; page 40 also emits
   `listing.completed`.
3. `sort-name` commits the full name order and its first viewport.
4. `filter-name` applies the case-sensitive `file-0001` substring filter and
   commits 90 ordered rows plus its first viewport.
5. `clear-filter` restores the name-sorted full order and first viewport.
6. `refresh` starts a new provider enumeration, then restores the name-sorted
   full order and first viewport.

The journey start barrier is an idle adapter with the fixture verified and no
location loaded. Each action uses the barrier rules above. It completes only
after `refresh` has no pending work and the final membership, name-order, and
viewport digests match. The validator rejects stale action ids, a commit after
its action completed, a missing page, or an output from a superseded action.
These are public input/event/view milestones. They do not claim a physical
frame commit.

Correlation, framing, sequence, timestamp monotonicity, row validity, output
integrity, and terminal status remain mandatory for every trace, including
`not_supported`, `error`, and `cancelled` results. A non-success result may
omit success-only milestones, so their absence alone does not produce
`missing_required_phase`. A declared capability failure has precedence when a
trace reports success: if the capability declaration does not include the
requested scenario, the normative direct `sample.completed` success example
returns `unsupported_reported_as_success`, even though it also omits every
success milestone. For a supported scenario, the same omission returns
`missing_required_phase`.

The following tables describe the later suite. They are not protocol v1
requirements unless a version 1 scenario above names them.

### Browse

| Scenario | Action | Milestones |
|---|---|---|
| `browse.fast.first` | Open `flat-10k` without metadata | first row, viewport, page, complete |
| `browse.fast.scale` | Open `flat-100k` | viewport, page, complete |
| `browse.metadata.first` | Open with size and timestamps | viewport, page, complete |
| `browse.next` | Request pages 2, 10, and final | page commit for each request |
| `browse.refresh` | Refresh a warm location | refreshed viewport and complete state |

### Presentation Pipeline

| Scenario | Action | Terminal condition |
|---|---|---|
| `view.sort.name` | Sort by name | correct visible order committed |
| `view.sort.size` | Sort by size | correct metadata-backed order committed |
| `view.filter.common` | Filter with 50 percent selectivity | full viewport committed |
| `view.filter.sparse` | Filter with 0.01 percent selectivity | terminal page or completion committed |
| `view.group.extension` | Group by extension | group labels, order, and viewport committed |
| `view.group.size` | Group by size | metadata-backed groups committed |

### Search

| Scenario | Action | Milestones |
|---|---|---|
| `search.early` | Find a root-near exact match | first match and completion |
| `search.late` | Find a deep late match | first match and completion |
| `search.common` | Match about 10 percent | first batch, viewport, completion |
| `search.none` | Match nothing | completion |
| `search.cancel` | Cancel after a fixed barrier | cancel accepted, work stopped, quiescent |
| `search.concurrent` | Run four session-isolated searches | per-session first match and completion |

### Whole-Pipeline Journeys

Journeys keep one application process alive and exercise the full state
machine. They expose cache, queue, rendering, and cancellation behavior that an
isolated operation misses.

`journey.browse-organize-search` performs:

1. Navigate to `flat-10k`.
2. Commit the first viewport.
3. Sort by size.
4. Group by extension.
5. Apply a sparse filter.
6. Clear the filter and request page 10.
7. Navigate into `tree-100k`.
8. Start a common search and render its first viewport.
9. Replace it with a rare search.
10. Cancel, navigate back, and refresh.

`journey.mutation-recovery` opens `mutation-10k`, applies the scripted
filesystem changes, and waits for the correct final view without duplicate or
missing rows.

`journey.decorated-browse` opens `git-10k`, commits undecorated listing rows,
then commits semantic decorations. Listing delivery must not wait for Git work.

## Rigorous Application Measurement

Application benchmarking uses an instrumented view and a black-box view at the
same time.

### Black-Box View

The driver launches or connects to the application, waits at a ready barrier,
injects a semantic action, and observes the first correct visible frame.

For terminal applications:

- use a pseudoterminal with fixed dimensions and terminal capabilities
- isolate `HOME`, XDG configuration, cache, and state directories
- parse ANSI output into a virtual terminal instead of matching raw bytes
- hash the visible rows, selection, group labels, and status state
- retain the terminal transcript when a sample fails

For a future graphical Filer application, use the platform accessibility tree
for semantic view correctness and a frame-commit marker for presentation time.
Screen-image matching is a diagnostic artifact, not the primary correctness
oracle.

The black-box metrics are fair across applications:

- process start to first correct frame
- ready application input to first changed correct frame
- input to stable terminal state
- peak resident memory and CPU time

### Instrumented Filer View

Filer also emits benchmark trace events for attribution. These events carry ids
and timestamps but do not change command or result semantics.

Use these phases:

1. `input.injected`
2. `command.sent`
3. `router.accepted`
4. `provider.started`
5. `provider.first_batch`
6. `provider.completed`
7. `pipeline.started`
8. `pipeline.completed`
9. `event.enqueued`
10. `event.received`
11. `view.committed`
12. `frame.committed`
13. `work.quiescent`

Derive provider, pipeline, queue, view-update, and render durations from one
monotonic clock domain. Propagate the run, action, session, and request ids
through every phase. Missing or duplicate phases required by the selected
scenario invalidate the sample. CORE-042 owns this internal attribution;
the initial CORE-032 journey requires only its public input/event/view markers.

The reference application is a deterministic consumer of public Filer-core
events. It maintains viewport, selection, sorting, filtering, grouping, and
search state, then commits a virtual frame. This gives Filer-core a rigorous
input-to-view benchmark independently of the app:UI-011 desktop validation
track. Keep its virtual-view measurements separate from that track's actual
window/frame observations. A later benchmark adapter can measure the real app.

### Responsiveness Under Work

Throughput alone does not prove that browsing feels fast. While a browse,
search, metadata, or decoration action runs, inject a lightweight focus or
selection action at a fixed interval.

Record:

- input-to-frame median, p95, and p99
- maximum main-loop stall
- event queue high-water mark
- frames committed and superseded
- stale events rejected
- time from cancel acceptance to quiescence
- results or filesystem work observed after cancellation

This measurement catches a fast total completion that still freezes input.

## Metrics

### Latency

- time to first row
- time to first viewport, default 40 rows
- time to first page, default 256 rows
- time to first search match and first search viewport
- time to complete
- input to correct frame
- cancellation acceptance to quiescence
- mutation to correct converged frame

### Work and Resources

- rows examined, accepted, emitted, and visible
- work amplification, `rows examined / rows visible`
- CPU time
- peak resident memory
- allocation count and allocated bytes when supported
- directory-read and metadata syscall counts when supported
- event and frame counts

### Reliability

- output digest and row-count agreement
- duplicate or missing rows across pages
- stale events after superseding actions
- work observed after cancellation
- final view generation after mutation

## Sampling and Cache Policy

Report cold-start, warm-start, and warm steady-state results separately.

- Cold-start samples use independent processes and isolated application state.
- Cold-filesystem samples use a fresh fixture copy or an explicitly recorded
  platform cache reset. Never require privileged cache dropping in normal CI.
- Warm samples run a declared warmup before timing.
- Steady-state journeys reuse one process but reset semantic state at a barrier.

Use at least 20 independent process samples for startup results. Use at least
five processes with ten randomized actions each for steady-state journeys.
Randomize implementation order within each round.

Report median, p95, p99 when the sample count supports it, median absolute
deviation, and a bootstrap confidence interval for the median. Store every raw
sample. A generated summary must be reproducible from raw JSON.

Record:

- operating system, kernel, CPU, logical CPU count, and power policy
- filesystem, mount options, and fixture location
- memory size and current background-load check
- compiler and build profile
- implementation versions, commits, and binary digests
- adapter configuration and declared capabilities

The raw result stores immutable machine, filesystem, build, and adapter profile
records. The request references the machine and filesystem records by id and
digest and carries the build and adapter identity directly. A machine record
contains OS, kernel, CPU model, physical and logical CPU counts, memory, power
policy, and virtualization state. A filesystem record contains filesystem type,
sorted mount options, block size, mount identity, fixture path class, and cache
preparation method. A build record contains source revision, compiler version,
target, features, build profile, and binary digest. An adapter record contains
adapter id, version, binary digest, configuration, and declared capabilities.
Profile digests use the canonical token algorithm with scope `profile`, fields
`name` and `value`, and records in the order listed here.

Do not infer cache state. `controlled_cold` requires a recorded platform cache
reset. `fresh_copy` requires a new fixture path and records whether its source
may still be cached. `uncontrolled` is never reported as cold. Warmup samples
are stored but marked ineligible. A steady-state sample records the warmup
action id and the semantic reset barrier.

Every requested resource metric appears on `sample.completed` as an observed
integer or an unavailable value. Reports show observed coverage as `n/N` and
group unavailable reasons. They never replace unavailable values with zero,
drop the implementation from correctness results, or estimate a value. A gate
that needs an unavailable metric is `not_evaluable`, not passed. Correctness
counts and digests required by a scenario remain mandatory even when resource
metrics are unavailable.

## Gates and Interpretation

Correctness and structural gates are portable:

- output digests match
- an unfiltered provider-order first page of 256 rows examines at most 512
  provider rows on the flat fixture
- sparse filters produce the correct matches and preserve continuation without
  a fixed examined-row ceiling; snapshot-only sorting/grouping stays explicit
- cancellation permits its documented terminal status and rejects stale success
  results after the scenario's cancellation barrier
- mutable views converge without duplicate or missing rows

The first-page examined-row gate applies only to an adapter that declares a
streaming listing capability and can observe examined rows at its public
boundary. An unavailable examined count makes that structural gate
`not_evaluable`. It does not invalidate an otherwise correct sample. A sparse
filter has no fixed examined-row ceiling because correct matches may occur at
the end of the source. It must instead preserve continuation, reach completion,
and match the filtered digest. A snapshot-only sort or group starts timing
after its complete input snapshot barrier and reports transform latency. It
cannot claim a streaming first-page result.

The unfiltered cap is owned by the streaming, provider-order first-page
scenario only. It is not applied to filtered or snapshot-only work, even when
those traces expose an examined count. CORE-042 owns the sparse fixture and its
end-to-end continuation proof. Version 1 therefore adds no unsupported sparse
scenario merely to exercise that future case.

Performance regression gates run on a reference machine. Begin with:

- median regression greater than 10 percent is a warning
- p95 regression greater than 15 percent is a warning
- the same regression reproduced in two clean runs is a failure

Competitor ratios are informational until the suite has three stable baselines.
After that, record a target per scenario instead of one global score. A useful
initial target is Filer first-page latency within 1.5 times the fastest
semantically equivalent framework adapter.

Do not hide a tradeoff in an aggregate score. Publish browse, search,
presentation, responsiveness, resource, and reliability results separately.

## Version 1 Conformance Matrix

CORE-039 implements each named validator test below through the adapter
request/event seam. Tests use golden JSON messages and generated flat fixtures,
not Filer internals. The runtime owner later proves that a real adapter can
produce a conforming trace. Rejection codes are stable version 1 result codes.

| Test | Invalid input or rejection rule | Required result code |
|---|---|---|
| `rejects_malformed_json` | A request or stdout line is not one complete JSON object | `malformed_json` |
| `rejects_unsupported_protocol_version` | Request or event version is not integer `1` | `unsupported_protocol_version` |
| `rejects_unknown_missing_or_duplicate_fields` | A strict object has an unknown, missing, or duplicate key | `invalid_schema` |
| `rejects_invalid_scalar_types_and_ranges` | A field has the wrong JSON type, an integer is out of range, or an id or digest has invalid syntax | `invalid_schema` |
| `rejects_invalid_scenario_configuration` | Fixture, fields, sort, filter, page, viewport, group, search, or cache state does not match the scenario | `invalid_scenario_configuration` |
| `rejects_fixture_reference_mismatch` | Fixture id or digest differs from the selected manifest | `fixture_reference_mismatch` |
| `rejects_reused_sample_identity` | `(run_id, sample_id)` was already accepted | `duplicate_sample` |
| `rejects_event_correlation_mismatch` | Any event correlation id differs from the request | `correlation_mismatch` |
| `rejects_sequence_gap_or_duplicate` | Sequence does not start at zero or advance by one | `invalid_sequence` |
| `rejects_monotonic_clock_regression` | A later sequence has a lower timestamp | `clock_regression` |
| `rejects_unknown_or_misordered_phase` | A phase is unknown, outside its action, or violates the scenario order | `invalid_phase` |
| `rejects_stale_or_unknown_action` | Action id is unknown, completed, or superseded | `invalid_action` |
| `rejects_inconsistent_counts` | Counts decrease or violate `accepted <= examined`, `emitted <= accepted`, or `visible <= emitted` | `invalid_counts` |
| `rejects_unavailable_correctness_count` | A scenario-required count uses an unavailable value | `required_count_unavailable` |
| `rejects_invalid_row_projection` | Identity is unsafe, kind or metadata is invalid, or requested fields are missing or extra | `invalid_row` |
| `rejects_duplicate_rows` | A commit repeats a row or a continuation chain repeats an identity | `duplicate_identity` |
| `rejects_output_row_count_mismatch` | Output count differs from the committed rows or required manifest count | `output_row_count_mismatch` |
| `rejects_wrong_output_digest` | A commit digest differs from canonical rows or a golden digest | `output_digest_mismatch` |
| `rejects_incomplete_membership` | Completed provider-order rows do not match the manifest membership | `membership_mismatch` |
| `rejects_missing_required_phase` | A success trace omits a scenario phase | `missing_required_phase` |
| `rejects_duplicate_required_phase` | A singleton phase or terminal event appears twice | `duplicate_phase` |
| `rejects_invalid_terminal_status` | Status is missing, appears before terminal, has the wrong shape, or terminal is not last | `invalid_status` |
| `rejects_unsupported_scenario_reported_as_success` | The adapter lacks a required capability but emits success | `unsupported_reported_as_success` |
| `rejects_non_event_stdout` | Standard output contains diagnostics or any non-event JSON object | `unexpected_stdout` |

The invalid request above is the golden case for `invalid_schema`. The
one-row, 256-count page event is the golden case for
`output_row_count_mismatch`. The following terminal event is a normative
`unsupported_reported_as_success` case when the adapter capability record does
not include `browse.refresh`:

```json
{"protocol_version":1,"type":"run_event","run_id":"run-local-001","sample_id":"sample-0001","process_id":"process-0001","order_id":"round-01-position-02","sequence":1,"timestamp_ns":4200,"phase":"sample.completed","action_id":null,"counts":{"examined":0,"accepted":0,"emitted":0,"visible":0},"rows":[],"output":null,"metrics":{},"status":{"kind":"success","code":null,"message":null}}
```

CORE-039 also implements one accepting golden trace and these initial scenario
gate tests:

| Test | Gate proven from the trace | Runtime owner |
|---|---|---|
| `accepts_fast_first_trace` | First row, 40-row viewport, 256-row page, 10,000-row completion, and membership | CORE-040 |
| `accepts_fast_scale_trace` | 40-row viewport, 256-row page, 100,000-row completion, and membership | CORE-040 |
| `accepts_metadata_first_trace` | Requested metadata projection, metadata digest, and no extra fields | CORE-040 and CORE-041 |
| `accepts_continuation_trace` | Pages 1 through 40, final 16 rows, continuation state, uniqueness, and membership | CORE-040 |
| `accepts_name_sort_trace` | Snapshot barrier, full name-order digest, and viewport digest | CORE-032 |
| `accepts_name_filter_trace` | Snapshot barrier, 90-row filtered digest, and viewport digest | CORE-032 |
| `accepts_refresh_trace` | New enumeration, membership, restored name order, and visible viewport | CORE-032 |
| `accepts_reference_journey_trace` | All action barriers, visible milestones, completion, and no stale outputs | CORE-032 |
| `classifies_streaming_first_page_gate` | Observed `examined <= 512` passes; a larger value fails; unavailable is `not_evaluable` | CORE-040 and CORE-041 |
| `does_not_apply_streaming_cap_to_sparse_filter` | Exact sparse result and continuation pass without a fixed examined-row ceiling | CORE-042 |
| `keeps_snapshot_transform_separate` | Sort timing starts after snapshot completion and cannot be ranked as streaming | CORE-032 |

CORE-042 owns the executable cases for tree, sparse-match, Git, hostile-name,
and mutation fixtures; search, cancellation, mutation-recovery, decorated
browse, responsiveness, and internal trace phases. Those additions must extend
this matrix with their versioned schema and golden values. They cannot change
version 1 meanings or make CORE-039 depend on the extended corpus.

## Result Storage

Store:

- versioned fixture manifests and expected digests
- adapter capability and version records
- raw JSON samples
- generated Markdown summaries
- traces and terminal transcripts only for failed samples

Keep machine-specific baselines in named directories. Do not overwrite old
results when a dependency, fixture, protocol, or machine profile changes.

## Implementation Boundary

Place the comparison runner and adapters in an isolated benchmark package under
`crates/filer-core/benchmarks/`. Keep its dependencies out of Filer-core production
and normal dev dependency graphs.

The 0.3.1 stages are:

1. CORE-030: protocol/schema examples, two flat manifests, and conformance design
2. CORE-039: executable validation and flat fixture generation
3. CORE-040 under CORE-031: isolated runner and Filer public-command adapter
4. CORE-041 under CORE-031: std/Tokio adapters, raw JSON, and generated reports
5. CORE-032: one reference-client browse journey and a recorded virtual-view baseline

CORE-042 later adds the remaining fixture corpus, search, mutation, decoration,
responsiveness, and internal trace attribution. CORE-034 adds GIO, KIO, and
recursive walkers after those fixtures exist. CORE-033 adds Yazi and Broot
independently of the system-framework adapters. All three are deferred without
a release milestone. Record missing capabilities explicitly when these stages
are selected.

## Reference Interfaces

- [GIO FileEnumerator](https://docs.gtk.org/gio/class.FileEnumerator.html)
- [KDE KCoreDirLister](https://api.kde.org/kcoredirlister.html)
- [Yazi](https://github.com/sxyazi/yazi)
- [Broot launch and command interface](https://dystroy.org/broot/launch/)
- [walkdir](https://github.com/BurntSushi/walkdir)
- [jwalk](https://docs.rs/jwalk/latest/jwalk/struct.WalkDirGeneric.html)
- [ignore WalkParallel](https://docs.rs/ignore/latest/ignore/struct.WalkParallel.html)
- [Hyperfine process benchmark runner](https://github.com/sharkdp/hyperfine)
