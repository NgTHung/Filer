# Sorted First Page Profile, 2026-09-30

A sorted first page of a 10,000-entry directory took 100 ms and 499,095 allocations, while a full unsorted snapshot of the same directory took 9.8 ms and 90,380 allocations. Explorer-style clients open folders sorted, so this gap decides how fast a folder opens. This record attributes the gap to call sites and measures PIPELINE-008's fix on one machine.

## Machine Profile

- OS: Linux 6.12.107+deb13-amd64, x86_64
- CPU: Intel Core i7-11800H, 16 logical CPUs
- Rust: rustc 1.98.1 (LLVM 22.1.8)
- Cargo: 1.98.1
- Filesystem: Btrfs on NVMe, compression zstd level 1
- Before revision: `969c987`, the natural name order from [PIPELINE-010](2026-09-30-pipeline-010-linux-i7-11800h-btrfs.md)
- After revision: `67f4541`

The benchmark settings and commands match the [PIPELINE-010 record](2026-09-30-pipeline-010-linux-i7-11800h-btrfs.md): 10,000 empty files, a 256-entry page, 3 warmups, and 20 timed samples for `large_directory`, and 100 samples after 5 warmups for `name_order`.

## Where the Time Went

No sampling profiler was available, so a temporary harness timed each stage of the sorted walk in a release build against 10,000 empty files, with `stats_alloc` counting allocations. The table shows the last of three rounds, which the round before it matched within 1.5 ms.

| Stage | Call site | ms | Allocations |
| --- | --- | ---: | ---: |
| Walk in 256-row offset pages | `walked_page` calling `LocalFs::list_page` | 80.5 | 498,695 |
| Walk one listing stream | `LocalFs::open_listing`, then `next_batch` | 7.7 | 90,400 |
| One full listing | `LocalFs::list` | 6.6 | 90,319 |
| Sort 10,000 rows by name | `SortBy` | 2.5 | 2 |

The offset walk explains almost all of the gap. `LocalFs::list_page` reopens `read_dir` on every call and skips the rows earlier pages returned, so 40 pages read about 200,000 entries. It accounts for 498,695 of the page's 499,095 allocations. The walked path used it for every provider that reports native paging, although `LocalFs` also offers a listing stream that reads each entry once.

The second cost was in the comparator. `compare_nodes` derived both rows' group sort keys on every comparison, even for ungrouped listings, and a label grouping allocated two labels per comparison. The `name_order` benchmark isolates it, because its rows arrive in random order. The fixture above arrives nearly sorted, so its sort costs less.

| Corpus | `SortBy` before, ms | `SortBy` after, ms | Allocations |
| --- | ---: | ---: | ---: |
| Mixed | 14.234 | 3.328 | 2 |
| Shared prefix | 14.142 | 3.577 | 2 |
| Non-ASCII | 14.587 | 4.195 | 2 |

Deriving group keys once per row cut 10.4 to 10.9 ms from each 10,000-row sort. A grouped sort by extension makes 20,011 allocations, one label per row in the sort stage and one in the grouping stage. Deriving labels per comparison made 304,645.

## Results

| Scenario | Revision | Median ms | p95 ms | Allocations | Allocated bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| Sorted first page | before | 100.058 | 104.578 | 499,095 | 37,275,579 |
| Sorted first page | after | 12.264 | 12.590 | 90,484 | 21,458,026 |
| Full snapshot | before | 9.773 | 9.908 | 90,380 | 10,330,700 |
| Full snapshot | after | 9.432 | 9.706 | 90,380 | 10,331,084 |

Sorted first-page p95 is 1.30 times full-snapshot p95. The sorted page makes 104 more allocations than the snapshot. The remaining 2.8 ms go to what only a sorted page does: deriving 10,000 name keys, sorting, and keeping up to 16,384 ordered rows for the next pages. The 11 MB of extra allocated bytes belong to those sort and retention buffers, which the snapshot does not need.
