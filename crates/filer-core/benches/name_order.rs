//! # Name Order Benchmark
//!
//! Measures what the default name order costs next to a plain byte comparison.
//! Every sorted listing pays this cost, so it bounds how fast a sorted first
//! page can be. The shipped `SortBy` stage derives each row's name key once into
//! a shared buffer, while a lone `compare_nodes` call derives two keys, so the
//! runner times both. Before timing, it proves `compare_nodes` is a strict
//! total order on edge-case names and that `SortBy` agrees with it on every
//! corpus.
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
use filer_core::pipeline::sort::{SortBy, SortField, SortOrder};
use filer_core::pipeline::{PipelineData, Stage};
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
    SortByStage,
    CompareNodes,
}

impl Candidate {
    const ALL: [Self; 3] = [Self::Bytes, Self::SortByStage, Self::CompareNodes];

    fn name(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::SortByStage => "sort_by",
            Self::CompareNodes => "compare_nodes",
        }
    }

    fn sort(self, rows: Vec<NodeEntry>, config: &PipelineConfig) -> Vec<NodeEntry> {
        let mut rows = rows;
        match self {
            Self::Bytes => rows.sort_unstable_by(|left, right| left.name.cmp(&right.name)),
            Self::SortByStage => {
                let sort = config.sort.unwrap_or_default();
                let stage = SortBy::new(sort.field, sort.order, sort.directories_first);
                return match stage.process(PipelineData::Flat(rows)) {
                    PipelineData::Flat(rows) => rows,
                    PipelineData::Grouped(grouped) => grouped
                        .groups
                        .into_iter()
                        .flat_map(|group| group.nodes)
                        .collect(),
                };
            }
            Self::CompareNodes => {
                rows.sort_unstable_by(|left, right| compare_nodes(config, left, right))
            }
        }
        rows
    }
}

/// A comparator that is not a strict total order can make the standard sort
/// panic and can skip or repeat rows across keyset continuations.
fn verify_total_order(config: &PipelineConfig) -> BenchResult<()> {
    let names: Vec<String> = EDGE_NAMES.iter().map(|name| name.to_string()).collect();
    let rows = rows_for(&names);
    let compare = |left: &NodeEntry, right: &NodeEntry| compare_nodes(config, left, right);
    for (a_index, a) in rows.iter().enumerate() {
        for (b_index, b) in rows.iter().enumerate() {
            let order = compare(a, b);
            if order != compare(b, a).reverse() {
                return Err(violation(format!(
                    "antisymmetry fails for {:?} and {:?}",
                    a.name, b.name
                )));
            }
            if (order == Ordering::Equal) != (a_index == b_index) {
                return Err(violation(format!("{:?} and {:?} tie", a.name, b.name)));
            }
            for c in &rows {
                if order.is_le() && compare(b, c).is_le() && compare(a, c).is_gt() {
                    return Err(violation(format!(
                        "transitivity fails for {:?}, {:?}, {:?}",
                        a.name, b.name, c.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn violation(detail: String) -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::other(format!("compare_nodes is not a total order: {detail}")).into()
}

/// Timings only compare implementations if both shipped paths agree on the
/// order.
fn verify_agreement(
    corpus: Corpus,
    rows: &[NodeEntry],
    config: &PipelineConfig,
) -> BenchResult<()> {
    let sorted_names = |candidate: Candidate| {
        candidate
            .sort(rows.to_vec(), config)
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>()
    };
    let expected = sorted_names(Candidate::CompareNodes);
    let actual = sorted_names(Candidate::SortByStage);
    if let Some(index) = expected.iter().zip(&actual).position(|(e, a)| e != a) {
        return Err(io::Error::other(format!(
            "sort_by disagrees with compare_nodes on {} at row {index}: {:?} vs {:?}",
            corpus.name(),
            actual[index],
            expected[index]
        ))
        .into());
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
    let sample = rows.to_vec();
    let region = Region::new(GLOBAL);
    let start = Instant::now();
    let sample = candidate.sort(sample, config);
    let elapsed = start.elapsed();
    let stats = region.change();
    black_box(&sample);
    Measurement {
        elapsed,
        allocations: stats.allocations,
        allocated_bytes: stats.bytes_allocated,
    }
}

fn print_examples(config: &PipelineConfig) {
    let examples: Vec<String> = [
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
    ]
    .map(str::to_string)
    .to_vec();
    let rows = rows_for(&examples);
    let order = |candidate: Candidate| {
        candidate
            .sort(rows.clone(), config)
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>()
    };
    println!("byte order:    {:?}", order(Candidate::Bytes));
    println!("natural order: {:?}", order(Candidate::SortByStage));
}

fn main() -> BenchResult<()> {
    let settings = Settings::from_environment()?;
    let config = PipelineConfig::default().sort(SortField::Name, SortOrder::Ascending, true);
    verify_total_order(&config)?;

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
    print_examples(&config);
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
