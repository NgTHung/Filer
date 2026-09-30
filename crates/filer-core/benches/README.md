# filer-core Benchmarks

These benchmarks tell you whether a change made directory listing or sorting slower. `large_directory` measures requests end to end through the public commands and events. `name_order` isolates the cost of comparing names, which every sorted listing pays once per comparison.

Both report minimum, median, p95, maximum, and mean latency, plus the median allocation count and allocated bytes per sample.

## Large directory

This benchmark measures how long `filer-core` takes to answer directory requests through its public commands and events. It tells you whether a change made the first page, continuations, or snapshots slower, and whether an active Git decoration request slows the listing.

```bash
cargo bench -p filer-core --bench large_directory
```

The runner creates a 10,000-entry directory inside an isolated Git worktree. It leaves fixture creation out of the timed samples and reports these scenarios:

- a fast first page with Git decorations off
- the same page while a `git.status` request is active
- a fast next page
- metadata and sorted first pages
- a fast full snapshot

Git must be on your `PATH`.

### Change the profile

The fixture goes in your temporary directory unless you set `FILER_BENCH_FIXTURE_ROOT`, which lets you measure a specific filesystem. `FILER_BENCH_ENTRIES`, `FILER_BENCH_PAGE_SIZE`, `FILER_BENCH_SAMPLES`, and `FILER_BENCH_WARMUP` change the rest of the profile. Keep the defaults when you compare against a recorded baseline.

Compare revisions on the same machine, filesystem, Rust toolchain, entry count, and page size. Timings from different machines do not compare.

### Structural gate

Timing alone cannot prove bounded work, because a fast machine hides extra rows. This test counts the rows a resumable provider stream yields through the public scan command:

```bash
cargo test -p filer-core --test large_directory_paging_test
```

It fails if a page reads more than its rows plus one lookahead. The decoration scenario waits for the Git backend to accept its request before it starts the listing timer, and waits for the decoration event only after the listing sample ends.

## Name order

This benchmark sorts 10,000 generated `NodeEntry` rows by name with each candidate name order, next to a plain byte comparison and the shipped `compare_nodes`. It runs three corpora: mixed ASCII and non-ASCII names, screenshot names that share a 16-byte prefix, and mostly non-ASCII names.

```bash
cargo bench -p filer-core --bench name_order
```

Before it times anything, the runner fails if a candidate is not a strict total order on a set of edge-case names, or if two candidates sort a corpus differently. A comparator that is not total can make the standard library sort panic and can skip or repeat rows across keyset continuations. `FILER_BENCH_ENTRIES`, `FILER_BENCH_SAMPLES`, and `FILER_BENCH_WARMUP` change the profile.

[ADR 0002](../../docs/adr/0002-default-name-order.md) records the run that chose the default name order.

## Baselines

[`baselines/`](baselines/) records each measured run with its machine profile and revision.
