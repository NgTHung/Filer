//! # Large Directory Benchmark
//!
//! Measures directory first paint and continuation through Filer-core's public
//! command and event contracts against a generated local filesystem fixture.
//! The generated fixture keeps inputs reproducible, and public contracts keep
//! timings aligned with the behavior clients observe.
//!
//! ```
//! use filer_core::DirectoryLoadOptions;
//!
//! assert!(DirectoryLoadOptions::page(256).is_paged());
//! ```

use std::error::Error;
use std::fs::{self, File};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::{Duration, Instant};

use filer_core::modules::git_decorations::{
    GitCliBackend, GitDecorationRequest, GitDecorationTarget, GitDecorationsModule,
    GitStatusBackend, GitStatusResult,
};
use filer_core::modules::scan::ScanModule;
use filer_core::{
    CancelSignal, Command, CoreError, DirectoryCursor, DirectoryLoadMode, DirectoryLoadOptions,
    Event, FilerCore, LocalFs, Location, LocationRef, PipelineConfig, RequestId,
};
use tempfile::TempDir;
use tokio::sync::Notify;

const DEFAULT_ENTRY_COUNT: usize = 10_000;
const DEFAULT_PAGE_SIZE: usize = 256;
const DEFAULT_SAMPLES: usize = 20;
const DEFAULT_WARMUP: usize = 3;
const EVENT_TIMEOUT: Duration = Duration::from_secs(30);

type BenchResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy)]
enum Scenario {
    FirstPageFast,
    FirstPageFastDecorations,
    NextPageFast,
    FirstPageMetadata,
    FirstPageSorted,
    FullSnapshotFast,
}

impl Scenario {
    const ALL: [Self; 6] = [
        Self::FirstPageFast,
        Self::FirstPageFastDecorations,
        Self::NextPageFast,
        Self::FirstPageMetadata,
        Self::FirstPageSorted,
        Self::FullSnapshotFast,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::FirstPageFast => "first_page_fast",
            Self::FirstPageFastDecorations => "first_page_fast_decorations",
            Self::NextPageFast => "next_page_fast",
            Self::FirstPageMetadata => "first_page_metadata",
            Self::FirstPageSorted => "first_page_sorted",
            Self::FullSnapshotFast => "full_snapshot_fast",
        }
    }
}

struct Settings {
    entry_count: usize,
    page_size: usize,
    samples: usize,
    warmup: usize,
    fixture_root: Option<PathBuf>,
}

impl Settings {
    fn from_environment() -> BenchResult<Self> {
        Ok(Self {
            entry_count: read_positive_usize("FILER_BENCH_ENTRIES", DEFAULT_ENTRY_COUNT)?,
            page_size: read_positive_usize("FILER_BENCH_PAGE_SIZE", DEFAULT_PAGE_SIZE)?,
            samples: read_positive_usize("FILER_BENCH_SAMPLES", DEFAULT_SAMPLES)?,
            warmup: read_positive_usize("FILER_BENCH_WARMUP", DEFAULT_WARMUP)?,
            fixture_root: std::env::var_os("FILER_BENCH_FIXTURE_ROOT").map(PathBuf::from),
        })
    }
}

struct Fixture {
    _directory: TempDir,
    listing_path: PathBuf,
    location: LocationRef,
}

impl Fixture {
    fn generate(entry_count: usize, fixture_root: Option<&Path>) -> BenchResult<Self> {
        let directory = match fixture_root {
            Some(root) => {
                fs::create_dir_all(root)?;
                tempfile::tempdir_in(root)?
            }
            None => tempfile::tempdir()?,
        };
        let listing_path = directory.path().join("entries");
        fs::create_dir(&listing_path)?;
        for index in 0..entry_count {
            File::create(listing_path.join(format!("entry_{index:05}.dat")))?;
        }
        initialize_git_repository(directory.path())?;
        let location = LocationRef::from_location(&Location::local(&listing_path));
        Ok(Self {
            _directory: directory,
            listing_path,
            location,
        })
    }

    fn path(&self) -> &Path {
        &self.listing_path
    }
}

struct DecorationBenchmarkBackend {
    git: GitCliBackend,
    started: Arc<Notify>,
}

impl DecorationBenchmarkBackend {
    fn new(started: Arc<Notify>) -> Self {
        Self {
            git: GitCliBackend::new(),
            started,
        }
    }
}

#[async_trait::async_trait]
impl GitStatusBackend for DecorationBenchmarkBackend {
    async fn status(
        &self,
        parent: &Path,
        visible: &[GitDecorationTarget],
        cancel: &CancelSignal,
    ) -> Result<GitStatusResult, CoreError> {
        self.started.notify_one();
        self.git.status(parent, visible, cancel).await
    }
}

struct Harness {
    core: FilerCore,
    events: flume::Receiver<Event>,
    session: filer_core::model::session::SessionId,
    location: LocationRef,
    location_path: PathBuf,
    decorations_started: Arc<Notify>,
    page_size: usize,
}

impl Harness {
    async fn new(location: LocationRef, page_size: usize) -> BenchResult<Self> {
        let location_path = location
            .descriptor()
            .and_then(|descriptor| descriptor.as_local_path())
            .map(Path::to_path_buf)
            .ok_or_else(|| io::Error::other("benchmark fixture must have a local descriptor"))?;
        let decorations_started = Arc::new(Notify::new());
        let core = FilerCore::new();
        core.load(ScanModule::new(std::sync::Arc::new(LocalFs::new())));
        core.load(GitDecorationsModule::with_backend(Arc::new(
            DecorationBenchmarkBackend::new(decorations_started.clone()),
        )));
        let events = core.event_receiver();
        core.send(Command::Handshake)?;
        let session = loop {
            if let Event::SessionCreated(session) = recv_event(&events).await? {
                break session;
            }
        };
        Ok(Self {
            core,
            events,
            session,
            location,
            location_path,
            decorations_started,
            page_size,
        })
    }

    async fn prepare(&self, scenario: Scenario) -> BenchResult<Option<DirectoryCursor>> {
        if !matches!(scenario, Scenario::NextPageFast) {
            return Ok(None);
        }
        let first = self
            .scan_page(PipelineConfig::default(), fast_page(self.page_size))
            .await?;
        first.cursor.map(Some).ok_or_else(|| {
            io::Error::other("first page completed without a continuation cursor").into()
        })
    }

    async fn run(
        &self,
        scenario: Scenario,
        prepared_cursor: Option<DirectoryCursor>,
    ) -> BenchResult<Measurement> {
        match scenario {
            Scenario::FirstPageFast => {
                self.measure_page(PipelineConfig::default(), fast_page(self.page_size))
                    .await
            }
            Scenario::FirstPageFastDecorations => self.scan_page_with_decorations().await,
            Scenario::NextPageFast => {
                let cursor = prepared_cursor
                    .ok_or_else(|| io::Error::other("next-page benchmark was not prepared"))?;
                self.measure_page(
                    PipelineConfig::default(),
                    DirectoryLoadOptions::page_after(self.page_size, cursor),
                )
                .await
            }
            Scenario::FirstPageMetadata => {
                self.measure_page(
                    PipelineConfig::default(),
                    DirectoryLoadOptions {
                        listing: filer_core::ListingOptions::metadata(),
                        mode: DirectoryLoadMode::Page {
                            limit: self.page_size,
                            cursor: None,
                        },
                    },
                )
                .await
            }
            Scenario::FirstPageSorted => {
                self.measure_page(
                    PipelineConfig::with_default_sort(),
                    fast_page(self.page_size),
                )
                .await
            }
            Scenario::FullSnapshotFast => self.measure_snapshot().await,
        }
    }

    async fn measure_page(
        &self,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
    ) -> BenchResult<Measurement> {
        measure(async {
            let page = self.scan_page(pipeline, load).await?;
            Ok(page.rows)
        })
        .await
    }

    async fn scan_page(
        &self,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
    ) -> BenchResult<PageObservation> {
        let request = RequestId::new();
        self.core.send(Command::Scan {
            location: self.location.clone(),
            session: self.session,
            pipeline,
            load,
            request,
        })?;

        loop {
            match recv_event(&self.events).await? {
                Event::DirectoryPageLoaded {
                    groups,
                    page,
                    request: event_request,
                    ..
                } if event_request == request => {
                    return Ok(PageObservation {
                        rows: groups.total_count,
                        cursor: page.next_cursor,
                    });
                }
                Event::Error {
                    message,
                    request: Some(event_request),
                    ..
                } if event_request == request => return Err(io::Error::other(message).into()),
                _ => {}
            }
        }
    }

    async fn scan_snapshot(&self) -> BenchResult<usize> {
        let request = RequestId::new();
        self.core.send(Command::Scan {
            location: self.location.clone(),
            session: self.session,
            pipeline: PipelineConfig::default(),
            load: DirectoryLoadOptions::unbounded(filer_core::ListingOptions::fast()),
            request,
        })?;

        loop {
            match recv_event(&self.events).await? {
                Event::DirectoryLoaded {
                    groups,
                    request: event_request,
                    ..
                } if event_request == request => return Ok(groups.total_count),
                Event::Error {
                    message,
                    request: Some(event_request),
                    ..
                } if event_request == request => return Err(io::Error::other(message).into()),
                _ => {}
            }
        }
    }

    async fn measure_snapshot(&self) -> BenchResult<Measurement> {
        measure(self.scan_snapshot()).await
    }

    async fn scan_page_with_decorations(&self) -> BenchResult<Measurement> {
        let scan_request = RequestId::new();
        let decoration_request = RequestId::new();
        let visible = (0..self.page_size)
            .map(|index| {
                LocationRef::from_location(&Location::local(
                    self.location_path.join(format!("entry_{index:05}.dat")),
                ))
            })
            .collect::<Vec<_>>();
        let visible_count = visible.len();
        let decorations_started = self.decorations_started.notified();
        self.core.send(Command::Extension {
            key: "git.status".to_string(),
            payload: Arc::new(GitDecorationRequest {
                parent: self.location.clone(),
                visible,
                request: decoration_request,
            }),
            session: self.session,
        })?;
        tokio::time::timeout(EVENT_TIMEOUT, decorations_started)
            .await
            .map_err(|_| io::Error::other("timed out waiting for Git decoration work to start"))?;
        let start = Instant::now();
        self.core.send(Command::Scan {
            location: self.location.clone(),
            session: self.session,
            pipeline: PipelineConfig::default(),
            load: fast_page(self.page_size),
            request: scan_request,
        })?;

        let mut page_rows = None;
        let mut listing_elapsed = None;
        let mut decorations_received = false;
        while listing_elapsed.is_none() || !decorations_received {
            match recv_event(&self.events).await? {
                Event::DirectoryPageLoaded {
                    groups, request, ..
                } if request == scan_request => {
                    page_rows = Some(groups.total_count);
                    listing_elapsed = Some(start.elapsed());
                }
                Event::FileDecorationsUpdated {
                    decorations,
                    request,
                    ..
                } if request == decoration_request => {
                    if decorations.len() != visible_count {
                        return Err(io::Error::other(format!(
                            "Git decoration benchmark returned {} rows for {visible_count} visible rows",
                            decorations.len()
                        ))
                        .into());
                    }
                    decorations_received = true;
                }
                Event::Error {
                    message,
                    request: Some(request),
                    ..
                } if request == scan_request || request == decoration_request => {
                    return Err(io::Error::other(message).into());
                }
                _ => {}
            }
        }

        Ok(Measurement {
            rows: page_rows.ok_or_else(|| io::Error::other("decoration benchmark page missing"))?,
            elapsed: listing_elapsed
                .ok_or_else(|| io::Error::other("decoration benchmark timing missing"))?,
        })
    }
}

struct PageObservation {
    rows: usize,
    cursor: Option<DirectoryCursor>,
}

struct Measurement {
    rows: usize,
    elapsed: Duration,
}

async fn measure(operation: impl Future<Output = BenchResult<usize>>) -> BenchResult<Measurement> {
    let start = Instant::now();
    let rows = operation.await?;
    Ok(Measurement {
        rows,
        elapsed: start.elapsed(),
    })
}

struct Summary {
    min: Duration,
    median: Duration,
    p95: Duration,
    max: Duration,
    mean: Duration,
}

impl Summary {
    fn from_samples(samples: &mut [Duration]) -> BenchResult<Self> {
        if samples.is_empty() {
            return Err(io::Error::other("benchmark produced no samples").into());
        }
        samples.sort_unstable();
        let median = samples[samples.len() / 2];
        let p95_index = ((samples.len() * 95).div_ceil(100)).saturating_sub(1);
        let total_nanos = samples
            .iter()
            .map(Duration::as_nanos)
            .fold(0u128, u128::saturating_add);
        let mean_nanos = total_nanos / samples.len() as u128;
        let mean_nanos = u64::try_from(mean_nanos)
            .map_err(|_| io::Error::other("mean duration exceeded u64 nanoseconds"))?;
        Ok(Self {
            min: samples[0],
            median,
            p95: samples[p95_index],
            max: samples[samples.len() - 1],
            mean: Duration::from_nanos(mean_nanos),
        })
    }
}

#[tokio::main]
async fn main() -> BenchResult<()> {
    let settings = Settings::from_environment()?;
    let fixture_start = Instant::now();
    let fixture = Fixture::generate(settings.entry_count, settings.fixture_root.as_deref())?;
    let fixture_duration = fixture_start.elapsed();
    let harness = Harness::new(fixture.location.clone(), settings.page_size).await?;

    print_profile(&settings, fixture.path(), fixture_duration);
    println!(
        "{:<24} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "scenario", "rows", "min_ms", "median_ms", "p95_ms", "max_ms", "mean_ms"
    );

    for scenario in Scenario::ALL {
        for _ in 0..settings.warmup {
            let cursor = harness.prepare(scenario).await?;
            let _ = harness.run(scenario, cursor).await?;
        }
        let mut samples = Vec::with_capacity(settings.samples);
        let mut rows = 0;
        for _ in 0..settings.samples {
            let cursor = harness.prepare(scenario).await?;
            let measurement = harness.run(scenario, cursor).await?;
            rows = measurement.rows;
            samples.push(measurement.elapsed);
        }
        let summary = Summary::from_samples(&mut samples)?;
        println!(
            "{:<24} {:>8} {:>10.3} {:>10.3} {:>10.3} {:>10.3} {:>10.3}",
            scenario.name(),
            rows,
            millis(summary.min),
            millis(summary.median),
            millis(summary.p95),
            millis(summary.max),
            millis(summary.mean),
        );
    }

    harness.core.shutdown().await?;
    Ok(())
}

async fn recv_event(events: &flume::Receiver<Event>) -> BenchResult<Event> {
    tokio::time::timeout(EVENT_TIMEOUT, events.recv_async())
        .await
        .map_err(|_| io::Error::other("timed out waiting for benchmark event"))?
        .map_err(|_| io::Error::other("benchmark event channel closed").into())
}

fn fast_page(page_size: usize) -> DirectoryLoadOptions {
    DirectoryLoadOptions::page(page_size)
}

fn initialize_git_repository(path: &Path) -> BenchResult<()> {
    let output = ProcessCommand::new("git")
        .args(["init", "-q"])
        .current_dir(path)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
    .into())
}

fn read_positive_usize(name: &str, default: usize) -> BenchResult<usize> {
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    let parsed = value.parse::<usize>()?;
    if parsed == 0 {
        return Err(io::Error::other(format!("{name} must be greater than zero")).into());
    }
    Ok(parsed)
}

fn print_profile(settings: &Settings, fixture_path: &Path, fixture_duration: Duration) {
    let logical_cpus = std::thread::available_parallelism()
        .map(|value| value.get().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("filer-core large-directory benchmark");
    println!(
        "profile: os={} arch={} logical_cpus={} entries={} page_size={} samples={} warmup={} fixture_root={} fixture_ms={:.3}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        logical_cpus,
        settings.entry_count,
        settings.page_size,
        settings.samples,
        settings.warmup,
        fixture_path.parent().unwrap_or(fixture_path).display(),
        millis(fixture_duration),
    );
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
