# Large Directory Benchmark

This benchmark measures how long `filer-core` takes to answer directory requests through its public commands and events. It tells you whether a change made the first page, continuations, or snapshots slower, and whether an active Git decoration request slows the listing.

## Run it

```bash
cargo bench -p filer-core --bench large_directory
```

The runner creates a 10,000-entry directory inside an isolated Git worktree. It leaves fixture creation out of the timed samples and reports minimum, median, p95, maximum, and mean latency for these scenarios:

- a fast first page with Git decorations off
- the same page while a `git.status` request is active
- a fast next page
- metadata and sorted first pages
- a fast full snapshot

Git must be on your `PATH`.

## Change the profile

The fixture goes in your temporary directory unless you set `FILER_BENCH_FIXTURE_ROOT`, which lets you measure a specific filesystem. `FILER_BENCH_ENTRIES`, `FILER_BENCH_PAGE_SIZE`, `FILER_BENCH_SAMPLES`, and `FILER_BENCH_WARMUP` change the rest of the profile. Keep the defaults when you compare against a recorded baseline.

Compare revisions on the same machine, filesystem, Rust toolchain, entry count, and page size. Timings from different machines do not compare.

## Structural gate

Timing alone cannot prove bounded work, because a fast machine hides extra rows. This test counts the rows a resumable provider stream yields through the public scan command:

```bash
cargo test -p filer-core --test large_directory_paging_test
```

It fails if a page reads more than its rows plus one lookahead. The decoration scenario waits for the Git backend to accept its request before it starts the listing timer, and waits for the decoration event only after the listing sample ends.

## Baselines

[`baselines/`](baselines/) records each measured run with its machine profile and revision.
