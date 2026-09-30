---
status: proposed
date: 2026-09-30
---

# Default directory name order

Name sort compares raw bytes, so `Zeta` sorts before `alpha` and `file10` sorts before `file2`. People who use Explorer-style file managers expect names to sort without regard to case and with numbers in numeric order. The same comparison decides where an ordered continuation resumes, so the order must also be a strict total order. We chose a case-insensitive, number-aware order that derives one sort key per row.

## The order

Name order compares two names in four steps. Each step runs only when the previous step ties.

1. Main key. Each character compares by its lowercase form from `char::to_lowercase`, so letters in every script compare without case. A run of ASCII digits compares as one number: leading zeros are ignored, a run with fewer significant digits is smaller, and runs of equal length compare digit by digit. Runs never parse into an integer, so runs of 30 digits order correctly.
2. Leading zeros. Aligned digit runs compare by their count of leading zeros, fewer first, so `file1` sorts before `file01`.
3. Raw bytes. Names that differ only in case get a fixed order, uppercase first.
4. Location. The existing location tie-breaker in [`order.rs`](../../filer-core/src/pipeline/order.rs) stays last, because a provider can report two rows with the same name, such as duplicate archive members.

A digit run sorts where the digit characters sit among other characters. Space, `-`, and `.` sort before numbers, and `_` and letters sort after them, so `file.txt` sorts before `file1.txt` and `file1.txt` before `file_1.txt`. Lowercasing places `_` before every letter.

Lowercasing instead of uppercasing keeps the key close to one character per character. Only `İ` (U+0130) lowercases to two characters, while 102 characters uppercase to several, such as `ß` to `SS`. Only `İ` and the Kelvin sign lowercase to ASCII, and no character lowercases to a digit, so the digit-run rule never sees a digit that case folding produced.

The order does not normalize Unicode, fold accents, or apply locale rules. After lowercasing, characters compare by code point, so `Ärger` sorts after `zebra`, and a precomposed `é` differs from `e` followed by a combining accent. Local names that are not valid UTF-8 reach the comparator as empty strings, because `NodeEntry::from_metadata` replaces them with `""`. They sort first and fall through to the location tie-breaker.

The order applies to `SortField::Name`, to `SortField::Type`, which compares names, and to the name tie-breaker behind every other sort field. Descending Name reverses steps 1 to 3 together, so a descending listing is the exact reverse of an ascending one. When Size, Modified, Created, or Extension values are equal, rows fall back to ascending name order. Extension values and group labels keep their byte comparison, and directories-first and group ordering do not change.

## Examples

| Case | Earlier | Later | Deciding step |
|---|---|---|---|
| Mixed case | `alpha` | `Zeta` | Main key; byte order reverses them |
| Mixed case | `zebra` | `Zeta` | Main key |
| Case only | `File1` | `file1` | Raw bytes |
| Punctuation | `_build` | `alpha` | Main key |
| Numbered | `file2` | `file10` | Main key |
| Numbered | `IMG_0010` | `IMG_100` | Main key, 10 before 100 |
| Numbered | `file.txt` | `file1.txt` | Main key, `.` before a number |
| Numbered | `file1.txt` | `file_1.txt` | Main key, a number before `_` |
| Long run | `12345678901234567890123` | `12345678901234567890124` | Main key |
| Leading zeros | `file1` | `file01` | Leading zeros |
| Leading zeros | `file01` | `file001` | Leading zeros |
| Leading zeros | `file1a` | `file01b` | Main key; zeros never override a later character |
| Non-ASCII case | `Été` | `été` | Raw bytes |
| Non-ASCII numbered | `Tài liệu 2` | `tài liệu 10` | Main key |
| Non-ASCII letters | `zebra` | `Ärger` | Main key, by code point |

## Sort keys

One key function defines the main key, so sorting and continuation cannot disagree. The key lowercases each character into UTF-8 bytes and writes each digit run as a marker byte, a fixed-width count of significant digits, and the significant digits. Comparing two keys byte by byte gives the main-key order.

Bulk sorts derive each row's key once, not once per comparison, and store every key in one shared buffer. A sort of 10,000 rows makes about 140,000 comparisons, so per-comparison work dominates. The shared buffer keeps key derivation to a few buffer growths instead of one allocation per row. `SortBy` and `PageSelection` use this path. A single comparison, such as a call to `compare_nodes`, may derive both keys on the spot.

## Cursors and continuations

An ordered continuation resumes after the last row it returned. A keyset rewalk keeps only rows that sort after that boundary row ([`selection.rs:87`](../../filer-core/src/modules/scan/paging/selection.rs)), and it uses the same comparison that sorted the page ([`selection.rs:102`](../../filer-core/src/modules/scan/paging/selection.rs)). Rows are neither skipped nor repeated only if that comparison is a strict total order: two distinct rows never compare equal, and the order is transitive. The four steps end in raw bytes and location, so distinct rows always differ. With keys, the rewalk derives the boundary row's key once and compares each walked row's key against it.

Since Rust 1.81, the standard library sorts may panic when a comparator is not a total order. A defect in the comparison could crash a listing, not only misorder it, so the implementation needs a test over every triple of an edge-case name set.

[Directory cursors](../../CONTEXT.md) live in memory and are single-use, so changing the default order invalidates no state across restarts. Within one process the default order is fixed. A comparison mode added later must travel in `PipelineConfig`, because the continuation check rejects a cursor whose stored pipeline differs from the request ([`paging/mod.rs:484`](../../filer-core/src/modules/scan/paging/mod.rs)).

## Cost

The [`name_order` benchmark](../../filer-core/benches/README.md) sorts 10,000 generated `NodeEntry` rows by name. It first proves each candidate is a strict total order on edge-case names and that every candidate produces the same order on each corpus. The run used revision `43550a0`, 100 samples, and 5 warmups:

- OS: Linux 6.12.107+deb13-amd64, x86_64
- CPU: Intel Core i7-11800H, 16 logical CPUs
- Rust: rustc 1.98.1 (LLVM 22.1.8)
- Command: `cargo bench -q -p filer-core --bench name_order`

Median milliseconds per sort, with median allocations per sort:

| Candidate | Mixed | Shared prefix | Non-ASCII | Allocations |
|---|---:|---:|---:|---:|
| Byte comparison of names | 1.354 | 1.330 | 1.404 | 0 |
| Shipped `compare_nodes` (bytes) | 12.391 | 12.577 | 12.398 | 0 |
| Chosen: keys in a shared buffer | 2.212 | 2.929 | 3.503 | 2 |
| Keys allocated per row | 2.633 | 3.340 | 4.070 | 10,001 |
| Compare in place, skip shared prefix | 3.193 | 4.319 | 7.562 | 0 |
| Compare in place from the start | 3.177 | 8.826 | 8.061 | 0 |

The mixed corpus combines numbered photos, unpadded numbers, mixed-case words, dotfiles, and some non-ASCII names. The shared-prefix corpus holds screenshot names that share a 16-byte prefix. The non-ASCII corpus holds mostly Vietnamese, Japanese, Cyrillic, Greek, and accented Latin names. The chosen key path has a p95 of 2.253, 2.978, and 3.654 ms on the three corpora.

The chosen order adds 0.9 to 2.1 ms to a 10,000-row sort over plain byte comparison. The shipped `compare_nodes` costs more than that on its own. In an uncommitted experiment, skipping the group sort key for ungrouped listings cut its sort from 12.4 ms to 1.8 ms, so it derives group keys twice per comparison even when nothing is grouped. [PIPELINE-008](../../.tasks/core/PIPELINE-008-cut-sorted-first-page-cost-on-large-directories.md) owns that cost. For scale, the sorted first page on the same machine takes 101 ms at the median ([baseline](../../filer-core/benches/baselines/2026-09-05-core-021-linux-i7-11800h-btrfs.md)).

## Alternatives

Keeping byte order leaves listings in an order people do not expect, and PIPELINE-008 would optimize an order that later changes.

Comparing names in place needs no allocation, but even with the shared prefix skipped it ran 1.4 to 2.2 times slower than shared-buffer keys. It was slowest on non-ASCII names, where each differing character needs a case-table lookup on every comparison. Allocating a key per row ran 14 to 19 percent slower than the shared buffer and added 10,001 allocations per sort.

Folding only ASCII case is cheaper, but `Été` and `été` would then sort apart, with names such as `Öl` between them.

Resolving leading zeros with the raw-byte step alone orders `File1`, `file01`, `file1`, mixing case and zero differences. The separate leading-zero step runs only when main keys tie. In a throwaway prototype on the mixed corpus it changed the sort time from 3.04 ms to 3.08 ms.

Locale-aware collation and Unicode normalization need a dependency and a locale choice. They belong with the comparison modes in [PIPELINE-002](../../.tasks/core/PIPELINE-002-evolve-directory-presentation-contracts.md).

## Consequences

Every sorted listing and ordered continuation changes order once the implementation ships. `filer-app` sends a sort field to core and does not sort rows itself, so it needs no change.

[PIPELINE-010](../../.tasks/core/PIPELINE-010-ship-the-natural-default-directory-name-order.md) implements this order as its own tested step, and PIPELINE-008 measures its before and after runs on the shipped order. User-selectable and locale-aware comparison modes stay with PIPELINE-002.
