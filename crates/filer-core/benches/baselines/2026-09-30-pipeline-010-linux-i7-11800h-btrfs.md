# Natural Name Order Comparison, 2026-09-30

This comparison measures what shipping the natural default name order from [ADR 0002](../../../../docs/adr/0002-default-name-order.md) costs sorted listings. It records PIPELINE-010's before and after runs of both benchmarks on one machine.

## Machine Profile

- OS: Linux 6.12.107+deb13-amd64, x86_64
- CPU: Intel Core i7-11800H, 16 logical CPUs
- Rust: rustc 1.98.1 (LLVM 22.1.8)
- Cargo: 1.98.1
- Filesystem: Btrfs on NVMe, compression zstd level 1
- Before revision: `68cc9e6`, byte order
- After revision: `cdaa827`, natural order, timed with the name-order runner committed alongside this record

The large-directory runs generated 10,000 empty `.dat` files in an isolated Git worktree and used a 256-entry page, 3 warmups, and 20 timed samples. Fixture creation was excluded from every sample.

```bash
FILER_BENCH_FIXTURE_ROOT=target/bench-fixtures cargo bench -q -p filer-core --bench large_directory
cargo bench -q -p filer-core --bench name_order
```

Timing columns report elapsed milliseconds. Allocation columns report the sample median from the benchmark's `stats_alloc` region.

## Large Directory

| Scenario | Revision | Median ms | p95 ms | Allocations | Allocated bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| Sorted first page | before | 98.819 | 108.522 | 499,092 | 33,363,547 |
| Sorted first page | after | 100.058 | 104.578 | 499,095 | 37,275,579 |
| Full snapshot | before | 9.837 | 10.221 | 90,380 | 10,330,444 |
| Full snapshot | after | 9.773 | 9.908 | 90,380 | 10,330,700 |

The sorted first page changes within run-to-run noise and adds 3 allocations. The 3.9 MB of extra allocated bytes are the sorter's key buffer and keyed row buffer, which hold every buffered row during a sort pass. The other 499,000 allocations do not come from the name comparison, which allocates twice per sort pass. [PIPELINE-008](../../../../.tasks/core/PIPELINE-008-cut-sorted-first-page-cost-on-large-directories.md) owns the remaining gap to the full snapshot.

## Name Order

The before run timed the prototype candidates from ADR 0002. The after run times the paths that ship: the `SortBy` stage, which derives each row's key once into a shared buffer, and a lone `compare_nodes` call, which derives two keys per comparison. Each sorts 10,000 rows, 100 samples after 5 warmups.

| Corpus | Path | Median ms | p95 ms | Allocations | Allocated bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| Mixed | bytes | 1.380 | 1.584 | 0 | 0 |
| Mixed | byte-order `compare_nodes`, before | 12.724 | 12.851 | 0 | 0 |
| Mixed | `SortBy`, after | 14.234 | 16.437 | 2 | 3,870,562 |
| Mixed | `compare_nodes`, after | 29.652 | 30.015 | 279,080 | 8,250,423 |
| Shared prefix | bytes | 1.361 | 1.567 | 0 | 0 |
| Shared prefix | byte-order `compare_nodes`, before | 12.956 | 13.093 | 0 | 0 |
| Shared prefix | `SortBy`, after | 14.142 | 14.320 | 2 | 4,340,000 |
| Shared prefix | `compare_nodes`, after | 55.704 | 56.166 | 281,548 | 23,086,936 |
| Non-ASCII | bytes | 1.443 | 1.657 | 0 | 0 |
| Non-ASCII | byte-order `compare_nodes`, before | 12.762 | 12.958 | 0 | 0 |
| Non-ASCII | `SortBy`, after | 14.587 | 14.734 | 2 | 4,188,920 |
| Non-ASCII | `compare_nodes`, after | 57.598 | 58.501 | 276,680 | 18,518,843 |

`SortBy` makes 2 allocations per sort pass, one shared key buffer and one keyed row buffer, so it allocates nothing per row. It costs 1.2 to 1.8 ms more than the byte-order `compare_nodes` it replaces, which matches the 0.9 to 2.1 ms ADR 0002 measured between shared-buffer keys and byte comparison. Most of its 14 ms is not name work: the ADR's shared-buffer candidate sorted the same corpora in 2.2 to 3.6 ms. The rest is `compare_nodes` overhead, mainly the group sort keys it derives on every comparison, which PIPELINE-008 owns.

A lone `compare_nodes` call allocates two keys, so bulk sorts must not use it. Only tests and single comparisons call it.
