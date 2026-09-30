# Filer benchmark protocol package

`filer-core-benchmarks` is an isolated package for protocol v1 validation and
the reproducible flat fixtures used by CORE-039. It is not a runner and it does
not measure an adapter.

## Commands

Run these commands from the repository root:

```bash
cargo fmt --manifest-path filer-core/benchmarks/Cargo.toml --all -- --check
cargo check --manifest-path filer-core/benchmarks/Cargo.toml --locked --all-targets
cargo clippy --manifest-path filer-core/benchmarks/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path filer-core/benchmarks/Cargo.toml --locked --all-targets
cargo test --manifest-path filer-core/benchmarks/Cargo.toml --locked --doc
```

The all-targets command includes the real flat-100k fixture preparation test.
The nested manifest and lockfile keep this package outside the root workspace.

## Public interfaces

`RunValidator` accepts one validated request, trusted capability declarations,
requested metric names, and newline-terminated event lines. It returns stable
protocol errors or a `ValidatedSample` after explicit EOF finalization. The
validator checks framing, correlation, sequence, timestamps, action barriers,
counts, row projections, canonical output digests, terminal status, and the
first-page structural gate.

`prepare_fixture` creates a new target directory from a validated manifest,
uses sparse file lengths, applies exact manifest modification times, and runs a
full readback before returning `PreparedFixture`. `prepare_fixture_from_path`
validates the manifest before creating the target. The prepared fixture owns
its directory. Call `PreparedFixture::close` to report cleanup failures; drop
also attempts best-effort cleanup.

## Conformance coverage

The tests cover strict golden request and event parsing, both normative
manifests and their independent digest oracles, all eight version 1 scenario
shapes, continuation pages with two provider orders, transform and journey
barriers, gate classifications, stable rejection codes, requested metrics,
and real flat-10k and flat-100k filesystem readback. Generated fixture files
are not committed.

Synthetic traces prove public event semantics only. They cannot prove that a
real adapter performed no early work, collected no extra metadata, used one
monotonic clock, or started a new operating-system enumeration. CORE-040,
CORE-032, and CORE-042 own those runtime and extended-fixture proofs.

## Fixture test evidence

The real fixture tests passed on 2026-09-17 with Linux 6.12.101 on x86_64 and
Btrfs with a 4096-byte block size. The implementation uses standard filesystem
and timestamp APIs and rejects readback when the host cannot represent a
manifest timestamp exactly.
