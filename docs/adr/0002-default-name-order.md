---
status: accepted
date: 2026-09-30
---

# Default directory name order

Name sort compares raw bytes, so `Zeta` sorts before `alpha` and `file10` sorts before `file2`. People who use Explorer-style file managers expect case-insensitive order with numbers in numeric order. The same comparison decides where an ordered continuation resumes, so it must also be a strict total order. We chose a case-insensitive, number-aware order that derives one sort key per row.

## The order

Each step runs only when the previous step ties.

1. Main key. Each character compares by its `char::to_lowercase` form. A run of ASCII digits compares as one number by value, without parsing, so leading zeros are ignored and runs of any length order correctly.
2. Leading zeros. Aligned digit runs with fewer leading zeros sort first.
3. Raw bytes. Names that differ only in case get a fixed order, uppercase first.
4. Location. The existing tie-breaker in [`order.rs`](../../filer-core/src/pipeline/order.rs) stays last, because a provider can report two rows with the same name, such as duplicate archive members.

A digit run sorts where digit characters sit, so space, `-`, and `.` sort before numbers, and `_` and letters sort after them. We lowercase instead of uppercase because only `İ` (U+0130) lowercases to two characters, while 102 characters uppercase to several, and no character lowercases to a digit.

The order does not normalize Unicode, fold accents, or apply locale rules, so `Ärger` sorts after `zebra` by code point. Local names that are not valid UTF-8 reach the comparator as empty strings, because `NodeEntry::from_metadata` replaces them with `""` ([CORE-045](../../.tasks/core/CORE-045-show-non-utf-8-file-names-instead-of-empty-names.md)).

The order applies to `SortField::Name`, `SortField::Type`, and the name tie-breaker behind every other sort field. Descending Name reverses steps 1 to 3, so it is the exact reverse of ascending. Extension values, group labels, directories-first, and group ordering do not change.

## Examples

| Earlier | Later | Deciding step |
|---|---|---|
| `alpha` | `Zeta` | Main key; byte order reverses them |
| `File1` | `file1` | Raw bytes |
| `_build` | `alpha` | Main key |
| `file2` | `file10` | Main key |
| `IMG_0010` | `IMG_100` | Main key, 10 before 100 |
| `file.txt` | `file1.txt` | Main key, `.` before a number |
| `file1.txt` | `file_1.txt` | Main key, a number before `_` |
| `file1` | `file01` | Leading zeros |
| `file1a` | `file01b` | Main key; zeros never override a later character |
| `Été` | `été` | Raw bytes |
| `Tài liệu 2` | `tài liệu 10` | Main key |
| `zebra` | `Ärger` | Main key, by code point |

## Sort keys

One key function defines the main key, so sorting and continuation cannot disagree. The key holds each lowercased character as UTF-8 and each digit run as a marker byte, a fixed-width count of significant digits, and those digits, so comparing keys byte by byte gives the main-key order.

A 10,000-row sort makes about 140,000 comparisons, so `SortBy` and `PageSelection` derive each row's key once into one shared buffer. A single comparison, such as a call to `compare_nodes`, may derive both keys on the spot.

## Cursors and continuations

A keyset rewalk keeps only rows that sort after the last row returned ([`selection.rs:87`](../../filer-core/src/modules/scan/paging/selection.rs)), using the comparison that sorted the page ([`selection.rs:102`](../../filer-core/src/modules/scan/paging/selection.rs)). Rows are neither skipped nor repeated only if distinct rows never compare equal and the order is transitive. The steps end in raw bytes and location, so distinct rows always differ. The rewalk derives the boundary row's key once. Since Rust 1.81, the standard sorts may panic on a comparator that is not a total order, so the implementation must test every triple of an edge-case name set.

[Directory cursors](../../CONTEXT.md) live in memory and are single-use, so the new order invalidates no state across restarts. A later comparison mode must travel in `PipelineConfig`, because the continuation check rejects a cursor whose stored pipeline differs from the request ([`paging/mod.rs:484`](../../filer-core/src/modules/scan/paging/mod.rs)).

## Cost

The [`name_order` benchmark](../../filer-core/benches/README.md) sorts 10,000 generated `NodeEntry` rows after proving each candidate is a total order and all candidates agree. The run used revision `43550a0`, 100 samples, 5 warmups, an Intel Core i7-11800H on Linux 6.12.107, and rustc 1.98.1 (`cargo bench -q -p filer-core --bench name_order`).

| Candidate, median ms | Mixed | Shared prefix | Non-ASCII | Allocations |
|---|---:|---:|---:|---:|
| Byte comparison of names | 1.354 | 1.330 | 1.404 | 0 |
| Shipped `compare_nodes` (bytes) | 12.391 | 12.577 | 12.398 | 0 |
| Chosen: keys in a shared buffer | 2.212 | 2.929 | 3.503 | 2 |
| Keys allocated per row | 2.633 | 3.340 | 4.070 | 10,001 |
| Compare in place, skip shared prefix | 3.193 | 4.319 | 7.562 | 0 |
| Compare in place from the start | 3.177 | 8.826 | 8.061 | 0 |

The chosen order adds 0.9 to 2.1 ms over byte comparison. The shipped `compare_nodes` costs more on its own: in an uncommitted experiment, skipping the group sort key for ungrouped listings cut its sort from 12.4 ms to 1.8 ms. [PIPELINE-008](../../.tasks/core/PIPELINE-008-cut-sorted-first-page-cost-on-large-directories.md) owns that cost.

## Alternatives

Keeping byte order leaves listings in an order people do not expect. Comparing in place avoids allocation but ran 1.4 to 2.2 times slower than shared-buffer keys, worst on non-ASCII names. ASCII-only case folding would sort `Été` and `été` apart. Resolving leading zeros through raw bytes alone orders `File1`, `file01`, `file1`, mixing case and zero differences, while the separate step costs about 1 percent. Locale-aware collation and Unicode normalization need a dependency and a locale choice, so they stay with [PIPELINE-002](../../.tasks/core/PIPELINE-002-evolve-directory-presentation-contracts.md).

## Consequences

Every sorted listing and ordered continuation changes order once [PIPELINE-010](../../.tasks/core/PIPELINE-010-ship-the-natural-default-directory-name-order.md) ships it as its own tested step. PIPELINE-008 then measures the shipped order. `filer-app` sends a sort field to core and does not sort rows itself, so it needs no change.
