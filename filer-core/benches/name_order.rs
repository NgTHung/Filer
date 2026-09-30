//! # Name Order Benchmark
//!
//! Measures what a case-insensitive, number-aware default name order costs next
//! to the byte order `compare_nodes` ships. Candidates sort real `NodeEntry`
//! rows, so element moves cost what they cost in a listing. Before timing, the
//! benchmark proves each candidate is a total order on edge-case names and that
//! every candidate produces the same order on each corpus, so the timings
//! compare one order implemented several ways.
//!
//! ```
//! use filer_core::PipelineConfig;
//! use filer_core::pipeline::sort::{SortField, SortOrder};
//!
//! let config = PipelineConfig::default().sort(SortField::Name, SortOrder::Ascending, true);
//! assert!(config.sort.is_some());
//! ```

mod support;

use std::alloc::System;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hint::black_box;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use filer_core::model::node::NodeKind;
use filer_core::pipeline::compare_nodes;
use filer_core::pipeline::sort::{SortField, SortOrder};
use filer_core::{Location, LocationRef, NodeEntry, PipelineConfig};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use support::{BenchResult, CountSummary, Summary, millis, read_positive_usize};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const DEFAULT_ENTRY_COUNT: usize = 10_000;
const DEFAULT_SAMPLES: usize = 100;
const DEFAULT_WARMUP: usize = 5;

const ASCII_WORDS: &[&str] = &[
    "alpha", "Beta", "gamma", "Delta", "report", "Invoice", "notes", "README", "Cargo", "main",
    "lib", "Config", "build", "Makefile", "photo", "Summary", "draft", "final", "Backup",
    "archive", "zeta", "Zebra", "index", "Module", "test", "Budget", "plan",
];
const NON_ASCII_WORDS: &[&str] = &[
    "Tài liệu",
    "Hình ảnh",
    "Báo cáo",
    "Ảnh chụp",
    "Đề cương",
    "Bài giảng",
    "Hợp đồng",
    "Nhật ký",
    "Âm nhạc",
    "Ước tính",
    "Kế hoạch",
    "Tổng kết",
    "写真",
    "資料",
    "Документ",
    "отчёт",
    "Έγγραφο",
    "Übersicht",
    "Straße",
    "Résumé",
    "Été",
    "ñandú",
    "Øresund",
];
const EXTENSIONS: &[&str] = &[
    "txt", "rs", "md", "jpg", "PNG", "toml", "pdf", "json", "tar.gz",
];

/// Names that stress case, digit runs, leading zeros, and non-ASCII folding.
const EDGE_NAMES: &[&str] = &[
    "",
    "a",
    "A",
    "_build",
    "alpha",
    "Alpha",
    "ALPHA",
    "Zeta",
    "zebra",
    "file",
    "file.txt",
    "file-2.txt",
    "file_1",
    "file 1",
    "file1",
    "File1",
    "file01",
    "FILE01",
    "file001",
    "file1a",
    "file01b",
    "file2",
    "file10",
    "x9",
    "x9y",
    "x09y",
    "IMG_0009",
    "IMG_0010",
    "IMG_100",
    "Ärger",
    "ärger",
    "Été",
    "été",
    "\u{212A}elvin",
    "kelvin",
    "\u{130}stanbul",
    "istanbul",
    "Straße",
    "strasse",
    "STRASSE",
    "Tài liệu 2",
    "tài liệu 10",
    "写真 1",
    "12345678901234567890123",
    "12345678901234567890124",
];

type NameComparator = fn(&str, &str) -> Ordering;

struct Settings {
    entry_count: usize,
    samples: usize,
    warmup: usize,
}

impl Settings {
    fn from_environment() -> BenchResult<Self> {
        Ok(Self {
            entry_count: read_positive_usize("FILER_BENCH_ENTRIES", DEFAULT_ENTRY_COUNT)?,
            samples: read_positive_usize("FILER_BENCH_SAMPLES", DEFAULT_SAMPLES)?,
            warmup: read_positive_usize("FILER_BENCH_WARMUP", DEFAULT_WARMUP)?,
        })
    }
}

/// Deterministic generator so every run sorts the same names.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.next() as usize % items.len()]
    }
}

#[derive(Clone, Copy)]
enum Corpus {
    Mixed,
    SharedPrefix,
    NonAscii,
}

impl Corpus {
    const ALL: [Self; 3] = [Self::Mixed, Self::SharedPrefix, Self::NonAscii];

    fn name(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::SharedPrefix => "shared_prefix",
            Self::NonAscii => "non_ascii",
        }
    }

    /// Unique names, because one directory cannot hold two entries with the
    /// same name.
    fn names(self, count: usize) -> Vec<String> {
        let mut rng = Lcg(match self {
            Self::Mixed => 42,
            Self::SharedPrefix => 7,
            Self::NonAscii => 1_234,
        });
        let mut seen = HashSet::with_capacity(count);
        let mut names = Vec::with_capacity(count);
        while names.len() < count {
            let name = self.generate(&mut rng, names.len());
            if seen.insert(name.clone()) {
                names.push(name);
            }
        }
        names
    }

    fn generate(self, rng: &mut Lcg, index: usize) -> String {
        match self {
            Self::Mixed => match rng.below(10) {
                0..=2 => format!("IMG_{:04}.{}", rng.below(10_000), rng.pick(EXTENSIONS)),
                3..=4 => format!("file{}.{}", rng.below(5_000), rng.pick(EXTENSIONS)),
                5..=7 => format!(
                    "{}_{}.{}",
                    rng.pick(ASCII_WORDS),
                    rng.pick(ASCII_WORDS),
                    rng.pick(EXTENSIONS)
                ),
                8 => format!(".{}{index}", rng.pick(ASCII_WORDS).to_lowercase()),
                _ => format!(
                    "{} {}.{}",
                    rng.pick(NON_ASCII_WORDS),
                    rng.below(100),
                    rng.pick(EXTENSIONS)
                ),
            },
            Self::SharedPrefix => format!(
                "Screenshot 2026-{:02}-{:02} at {:02}.{:02}.{:02}.png",
                1 + rng.below(12),
                1 + rng.below(28),
                rng.below(24),
                rng.below(60),
                rng.below(60)
            ),
            Self::NonAscii => {
                let first = rng.pick(NON_ASCII_WORDS);
                let first = if rng.below(4) == 0 {
                    first.to_uppercase()
                } else {
                    first.to_string()
                };
                format!(
                    "{first} {} {}.{}",
                    rng.pick(NON_ASCII_WORDS),
                    rng.below(500),
                    rng.pick(EXTENSIONS)
                )
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Candidate {
    Bytes,
    ShippedCompareNodes,
    NaturalWalk,
    NaturalPrefixSkip,
    NaturalKeyPerRow,
    NaturalKeyArena,
}

impl Candidate {
    const ALL: [Self; 6] = [
        Self::Bytes,
        Self::ShippedCompareNodes,
        Self::NaturalWalk,
        Self::NaturalPrefixSkip,
        Self::NaturalKeyPerRow,
        Self::NaturalKeyArena,
    ];
    const NATURAL: [Self; 4] = [
        Self::NaturalWalk,
        Self::NaturalPrefixSkip,
        Self::NaturalKeyPerRow,
        Self::NaturalKeyArena,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::ShippedCompareNodes => "compare_nodes",
            Self::NaturalWalk => "natural_walk",
            Self::NaturalPrefixSkip => "natural_prefix_skip",
            Self::NaturalKeyPerRow => "natural_key_per_row",
            Self::NaturalKeyArena => "natural_key_arena",
        }
    }

    fn comparator(self) -> Option<NameComparator> {
        match self {
            Self::Bytes => Some(|left, right| left.cmp(right)),
            Self::NaturalWalk => Some(natural_walk),
            Self::NaturalPrefixSkip => Some(natural_prefix_skip),
            Self::NaturalKeyPerRow | Self::NaturalKeyArena => Some(natural_key_compare),
            Self::ShippedCompareNodes => None,
        }
    }

    fn sort(self, rows: &mut Vec<NodeEntry>, config: &PipelineConfig) {
        match self {
            Self::Bytes => rows.sort_unstable_by(|left, right| left.name.cmp(&right.name)),
            Self::ShippedCompareNodes => {
                rows.sort_unstable_by(|left, right| compare_nodes(config, left, right))
            }
            Self::NaturalWalk => {
                rows.sort_unstable_by(|left, right| natural_walk(&left.name, &right.name))
            }
            Self::NaturalPrefixSkip => {
                rows.sort_unstable_by(|left, right| natural_prefix_skip(&left.name, &right.name))
            }
            Self::NaturalKeyPerRow => {
                let mut keyed: Vec<(Vec<u8>, NodeEntry)> = rows
                    .drain(..)
                    .map(|row| (natural_key(&row.name), row))
                    .collect();
                keyed.sort_unstable_by(|(left_key, left), (right_key, right)| {
                    left_key
                        .cmp(right_key)
                        .then_with(|| tie_break(&left.name, &right.name))
                });
                rows.extend(keyed.into_iter().map(|(_, row)| row));
            }
            Self::NaturalKeyArena => {
                // One shared buffer holds every key, so deriving keys costs a
                // few buffer growths instead of one allocation per row.
                let mut arena = Vec::with_capacity(rows.iter().map(|row| row.name.len() + 4).sum());
                let mut keyed: Vec<((usize, usize), NodeEntry)> = rows
                    .drain(..)
                    .map(|row| {
                        let start = arena.len();
                        push_natural_key(&row.name, &mut arena);
                        ((start, arena.len()), row)
                    })
                    .collect();
                keyed.sort_unstable_by(
                    |((left_start, left_end), left), ((right_start, right_end), right)| {
                        arena[*left_start..*left_end]
                            .cmp(&arena[*right_start..*right_end])
                            .then_with(|| tie_break(&left.name, &right.name))
                    },
                );
                rows.extend(keyed.into_iter().map(|(_, row)| row));
            }
        }
    }
}

fn natural_walk(left: &str, right: &str) -> Ordering {
    natural_from(left, right, 0).then_with(|| tie_break(left, right))
}

fn natural_prefix_skip(left: &str, right: &str) -> Ordering {
    natural_from(left, right, token_prefix_len(left, right)).then_with(|| tie_break(left, right))
}

/// Allocates two keys per call, so only the correctness checks use it.
fn natural_key_compare(left: &str, right: &str) -> Ordering {
    natural_key(left)
        .cmp(&natural_key(right))
        .then_with(|| tie_break(left, right))
}

/// Case-insensitive, number-aware comparison starting at byte `start`, which
/// must begin a token in both names.
fn natural_from(left: &str, right: &str, start: usize) -> Ordering {
    let (left_bytes, right_bytes) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (start, start);
    loop {
        let (Some(&l), Some(&r)) = (left_bytes.get(i), right_bytes.get(j)) else {
            return (i < left_bytes.len()).cmp(&(j < right_bytes.len()));
        };
        if l.is_ascii_digit() && r.is_ascii_digit() {
            let (left_end, right_end) =
                (digit_run_end(left_bytes, i), digit_run_end(right_bytes, j));
            let order = compare_digit_values(&left_bytes[i..left_end], &right_bytes[j..right_end]);
            if order.is_ne() {
                return order;
            }
            (i, j) = (left_end, right_end);
        } else if l.is_ascii() && r.is_ascii() {
            let order = l.to_ascii_lowercase().cmp(&r.to_ascii_lowercase());
            if order.is_ne() {
                return order;
            }
            (i, j) = (i + 1, j + 1);
        } else {
            // Offsets only advance by whole characters or digit runs, so both
            // slices start on a character boundary.
            let (Some(lc), Some(rc)) = (left[i..].chars().next(), right[j..].chars().next()) else {
                return Ordering::Equal;
            };
            // Identical characters lowercase identically, so skip the table lookup.
            if lc != rc {
                let order = lc.to_lowercase().cmp(rc.to_lowercase());
                if order.is_ne() {
                    return order;
                }
            }
            (i, j) = (i + lc.len_utf8(), j + rc.len_utf8());
        }
    }
}

/// Resolves names the main comparison treats as equal: fewer leading zeros
/// first, then raw bytes, so the order stays total.
fn tie_break(left: &str, right: &str) -> Ordering {
    leading_zero_order(left, right).then_with(|| left.cmp(right))
}

/// Compares leading-zero counts of aligned digit runs. Only called on names
/// whose main comparison tied, so their digit runs line up.
fn leading_zero_order(left: &str, right: &str) -> Ordering {
    let (left_bytes, right_bytes) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    loop {
        while i < left_bytes.len() && !left_bytes[i].is_ascii_digit() {
            i += 1;
        }
        while j < right_bytes.len() && !right_bytes[j].is_ascii_digit() {
            j += 1;
        }
        if i >= left_bytes.len() || j >= right_bytes.len() {
            return Ordering::Equal;
        }
        let (left_end, right_end) = (digit_run_end(left_bytes, i), digit_run_end(right_bytes, j));
        let left_zeros = left_end - i - strip_leading_zeros(&left_bytes[i..left_end]).len();
        let right_zeros = right_end - j - strip_leading_zeros(&right_bytes[j..right_end]).len();
        if left_zeros != right_zeros {
            return left_zeros.cmp(&right_zeros);
        }
        (i, j) = (left_end, right_end);
    }
}

/// Length of the shared byte prefix that ends on a token boundary in both
/// names. Identical bytes lowercase identically, but a character or digit run
/// that crosses the first difference must be compared whole.
fn token_prefix_len(left: &str, right: &str) -> usize {
    let (left_bytes, right_bytes) = (left.as_bytes(), right.as_bytes());
    let mut end = left_bytes
        .iter()
        .zip(right_bytes)
        .take_while(|(l, r)| l == r)
        .count();
    while !(left.is_char_boundary(end) && right.is_char_boundary(end)) {
        end -= 1;
    }
    while end > 0 && left_bytes[end - 1].is_ascii_digit() {
        end -= 1;
    }
    end
}

fn natural_key(name: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(name.len() + 4);
    push_natural_key(name, &mut key);
    key
}

/// Encodes the main comparison as bytes, so sorting compares keys with memcmp.
fn push_natural_key(name: &str, key: &mut Vec<u8>) {
    let bytes = name.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        let byte = bytes[offset];
        if byte.is_ascii_digit() {
            let end = digit_run_end(bytes, offset);
            let digits = strip_leading_zeros(&bytes[offset..end]);
            // The marker places numbers where digits sit among characters, and
            // the fixed-width length sorts longer values after shorter ones.
            key.push(b'0');
            key.extend_from_slice(
                &u32::try_from(digits.len())
                    .unwrap_or(u32::MAX)
                    .to_be_bytes(),
            );
            key.extend_from_slice(digits);
            offset = end;
        } else if byte.is_ascii() {
            key.push(byte.to_ascii_lowercase());
            offset += 1;
        } else {
            let Some(character) = name[offset..].chars().next() else {
                break;
            };
            let mut buffer = [0; 4];
            for lower in character.to_lowercase() {
                key.extend_from_slice(lower.encode_utf8(&mut buffer).as_bytes());
            }
            offset += character.len_utf8();
        }
    }
}

fn digit_run_end(bytes: &[u8], start: usize) -> usize {
    start
        + bytes[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
}

fn strip_leading_zeros(digits: &[u8]) -> &[u8] {
    let zeros = digits.iter().take_while(|&&digit| digit == b'0').count();
    &digits[zeros..]
}

/// Compares digit runs by value without parsing, so runs longer than any
/// integer type still order correctly.
fn compare_digit_values(left: &[u8], right: &[u8]) -> Ordering {
    let (left, right) = (strip_leading_zeros(left), strip_leading_zeros(right));
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

/// A comparator that is not a strict total order can make the standard sort
/// panic and can skip or repeat rows across keyset continuations.
fn verify_total_order(candidate: Candidate, compare: NameComparator) -> BenchResult<()> {
    for &a in EDGE_NAMES {
        for &b in EDGE_NAMES {
            let order = compare(a, b);
            if order != compare(b, a).reverse() {
                return Err(violation(
                    candidate,
                    format!("antisymmetry fails for {a:?} and {b:?}"),
                ));
            }
            if (order == Ordering::Equal) != (a == b) {
                return Err(violation(candidate, format!("{a:?} and {b:?} tie")));
            }
            for &c in EDGE_NAMES {
                if order.is_le() && compare(b, c).is_le() && compare(a, c).is_gt() {
                    return Err(violation(
                        candidate,
                        format!("transitivity fails for {a:?}, {b:?}, {c:?}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn violation(candidate: Candidate, detail: String) -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::other(format!(
        "{} is not a total order: {detail}",
        candidate.name()
    ))
    .into()
}

/// Timings only compare implementations if their timed sort paths agree on
/// the order.
fn verify_agreement(
    corpus: Corpus,
    rows: &[NodeEntry],
    config: &PipelineConfig,
) -> BenchResult<()> {
    let sorted_names = |candidate: Candidate| {
        let mut sorted = rows.to_vec();
        candidate.sort(&mut sorted, config);
        sorted.into_iter().map(|row| row.name).collect::<Vec<_>>()
    };
    let expected = sorted_names(Candidate::NaturalWalk);
    for candidate in Candidate::NATURAL {
        let actual = sorted_names(candidate);
        if let Some(index) = expected.iter().zip(&actual).position(|(e, a)| e != a) {
            return Err(io::Error::other(format!(
                "{} disagrees with natural_walk on {} at row {index}: {:?} vs {:?}",
                candidate.name(),
                corpus.name(),
                actual[index],
                expected[index]
            ))
            .into());
        }
    }
    Ok(())
}

fn rows_for(names: &[String]) -> Vec<NodeEntry> {
    let parent = PathBuf::from("/filer-bench/names");
    names
        .iter()
        .map(|name| {
            NodeEntry::from_location_ref(
                LocationRef::from_location(&Location::local(parent.join(name))),
                name.as_str(),
                NodeKind::File { extension: None },
            )
        })
        .collect()
}

struct Measurement {
    elapsed: Duration,
    allocations: usize,
    allocated_bytes: usize,
}

fn measure(candidate: Candidate, rows: &[NodeEntry], config: &PipelineConfig) -> Measurement {
    let mut sample = rows.to_vec();
    let region = Region::new(GLOBAL);
    let start = Instant::now();
    candidate.sort(&mut sample, config);
    let elapsed = start.elapsed();
    let stats = region.change();
    black_box(&sample);
    Measurement {
        elapsed,
        allocations: stats.allocations,
        allocated_bytes: stats.bytes_allocated,
    }
}

fn print_examples() {
    let mut examples = vec![
        "file10",
        "file2",
        "File1",
        "file01",
        "file1",
        "Zeta",
        "alpha",
        "_build",
        "Ärger",
        "zebra",
        "IMG_100",
        "IMG_0009",
        "IMG_0010",
        "file.txt",
        "file1.txt",
        "file-1.txt",
        "file_1.txt",
        "tài liệu 10",
        "Tài liệu 2",
        "Été",
        "été",
    ];
    examples.sort_unstable();
    println!("byte order:    {examples:?}");
    examples.sort_unstable_by(|left, right| natural_walk(left, right));
    println!("natural order: {examples:?}");
}

fn main() -> BenchResult<()> {
    let settings = Settings::from_environment()?;
    for candidate in Candidate::NATURAL {
        if let Some(compare) = candidate.comparator() {
            verify_total_order(candidate, compare)?;
        }
    }

    let logical_cpus = std::thread::available_parallelism()
        .map(|value| value.get().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("filer-core name-order benchmark");
    println!(
        "profile: os={} arch={} logical_cpus={} entries={} samples={} warmup={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        logical_cpus,
        settings.entry_count,
        settings.samples,
        settings.warmup,
    );
    print_examples();
    println!(
        "{:<14} {:<20} {:>9} {:>10} {:>9} {:>9} {:>9} {:>9} {:>10} {:>10}",
        "corpus",
        "candidate",
        "min_ms",
        "median_ms",
        "p95_ms",
        "max_ms",
        "mean_ms",
        "vs_bytes",
        "alloc_med",
        "bytes_med"
    );

    let config = PipelineConfig::default().sort(SortField::Name, SortOrder::Ascending, true);
    for corpus in Corpus::ALL {
        let rows = rows_for(&corpus.names(settings.entry_count));
        verify_agreement(corpus, &rows, &config)?;
        let mut bytes_median = None;
        for candidate in Candidate::ALL {
            for _ in 0..settings.warmup {
                measure(candidate, &rows, &config);
            }
            let mut samples = Vec::with_capacity(settings.samples);
            let mut allocation_samples = Vec::with_capacity(settings.samples);
            let mut allocated_byte_samples = Vec::with_capacity(settings.samples);
            for _ in 0..settings.samples {
                let measurement = measure(candidate, &rows, &config);
                samples.push(measurement.elapsed);
                allocation_samples.push(measurement.allocations);
                allocated_byte_samples.push(measurement.allocated_bytes);
            }
            let summary = Summary::from_samples(&mut samples)?;
            let allocation_summary = CountSummary::from_samples(&mut allocation_samples)?;
            let allocated_byte_summary = CountSummary::from_samples(&mut allocated_byte_samples)?;
            if candidate == Candidate::Bytes {
                bytes_median = Some(summary.median);
            }
            let ratio = bytes_median
                .map(|bytes| summary.median.as_secs_f64() / bytes.as_secs_f64())
                .unwrap_or(f64::NAN);
            println!(
                "{:<14} {:<20} {:>9.3} {:>10.3} {:>9.3} {:>9.3} {:>9.3} {:>8.1}x {:>10} {:>10}",
                corpus.name(),
                candidate.name(),
                millis(summary.min),
                millis(summary.median),
                millis(summary.p95),
                millis(summary.max),
                millis(summary.mean),
                ratio,
                allocation_summary.median,
                allocated_byte_summary.median,
            );
        }
    }
    Ok(())
}
