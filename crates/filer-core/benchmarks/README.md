# Filer benchmark package

`filer-core-benchmarks` measures Filer-core and comparison adapters under one
versioned protocol, then rejects any sample whose output is wrong before its
timings can count. It validates protocol v1 traces, prepares reproducible
flat fixtures, runs each adapter as a supervised process, and stores one raw
JSON record per sample. The [benchmark specification](../../../docs/benchmarks/comparative-performance.md)
defines the protocol, scenarios, and gates.

The package has its own workspace and lockfile, so its dependencies never
enter Filer-core's production or normal development builds.

## Commands

Run these commands from the repository root:

```bash
cargo fmt --manifest-path crates/filer-core/benchmarks/Cargo.toml --all -- --check
cargo check --manifest-path crates/filer-core/benchmarks/Cargo.toml --locked --all-targets
cargo clippy --manifest-path crates/filer-core/benchmarks/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path crates/filer-core/benchmarks/Cargo.toml --locked --all-targets
cargo test --manifest-path crates/filer-core/benchmarks/Cargo.toml --locked --doc
```

The all-targets tests prepare real flat-10k and flat-100k fixtures and run the
Filer adapter against them through the runner.

## Validate a trace

`RunValidator` accepts one request, trusted capability declarations,
requested metric names, and newline-terminated event lines. It returns a
stable protocol error or, after explicit EOF finalization, a `ValidatedSample`.
It checks framing, correlation, sequence, timestamps, action barriers, counts,
row projections, canonical output digests, terminal status, and the
first-page structural gate. The validated timeline holds each accepted event
without its rows, so timings come only from validated events.

Golden messages and synthetic traces cover every version 1 rejection code and
scenario shape. They prove event semantics only. The Filer adapter tests prove
that a real adapter produces conforming traces on generated fixtures.

## Run a sample

`Runner` takes a `RunPlan` with the trusted inputs for one run: build,
machine, and filesystem profile records, the adapter program and its declared
capabilities, the cache state, requested metric names, a per-sample deadline,
and a results directory. It borrows a `PreparedFixture` that already passed
readback.

`Runner::run_sample` builds the request, starts a fresh adapter process, and
streams every stdout line through `RunValidator`. A rejected line, a missed
deadline, or a failed exit stops and reaps the adapter at once. The returned
`SampleRecord` is timing eligible only when the validator accepted the whole
trace and the adapter reported `success`. Rejected records keep the raw
stdout lines and up to 1 MiB of stderr.

Each record is written to `<results>/<run_id>/<sample_id>.json` and is never
overwritten. It holds the exact request, the full profile records behind the
request's digests, the fixture identity, and either the validated row-free
timeline with metrics or the rejection code with its diagnostics.

## Adapters

`filer-public-adapter` measures Filer-core through `Command` and `Event` only.
It starts the default core and a session before `sample.started` and supports
`browse.fast.first`, `browse.fast.scale`, `browse.metadata.first`, and
`browse.next`. Other scenarios report `not_supported`. Filer-core delivers
rows a page at a time, so `row.first`, the viewport, and the first page share
the page's arrival timestamp. Public events do not expose examined rows, so
`examined` is `not_observable` and the first-page gate is `not_evaluable`.

The adapter reports these metrics when requested. Any other name is
`unsupported`.

| Metric | Meaning |
|---|---|
| `cpu_time_ns` | Process CPU time between `sample.started` and the last action |
| `peak_rss_bytes` | Process peak resident memory, including untimed startup |
| `core_event_count` | Public core events received during the measured actions |

New adapters use `AdapterArgs`, `AdapterTrace`, and `ResourceMeter`.
`AdapterTrace` assigns sequence numbers, sample totals, row projections, and
digests after the measured actions, so encoding never delays a milestone.

`filer-bench-replay` replays a recorded trace as an adapter process. The
runner tests use it to exercise hangs, stderr diagnostics, and exit codes.

## Fixtures

`prepare_fixture` creates a new target directory from a validated manifest,
uses sparse file lengths, applies exact manifest modification times, and runs a
full readback before returning `PreparedFixture`. The prepared fixture owns its
directory. Call `PreparedFixture::close` to report cleanup failures; drop also
attempts best-effort cleanup. Generated fixtures are not committed.

The real fixture tests passed on 2026-09-17 with Linux 6.12.101 on x86_64 and
Btrfs with a 4096-byte block size. The implementation uses standard filesystem
and timestamp APIs and rejects readback when the host cannot represent a
manifest timestamp exactly.
