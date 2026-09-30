//! # Benchmark Support
//!
//! Sample summaries and environment settings shared by the `filer-core`
//! benchmarks. Every benchmark reports the same statistics so runs recorded in
//! different baselines stay comparable.
//!
//! ```ignore
//! let mut samples = vec![Duration::from_millis(2), Duration::from_millis(1)];
//! let summary = Summary::from_samples(&mut samples)?;
//! assert_eq!(summary.min, Duration::from_millis(1));
//! ```

use std::error::Error;
use std::io;
use std::time::Duration;

pub type BenchResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct Summary {
    pub min: Duration,
    pub median: Duration,
    pub p95: Duration,
    pub max: Duration,
    pub mean: Duration,
}

impl Summary {
    pub fn from_samples(samples: &mut [Duration]) -> BenchResult<Self> {
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

pub struct CountSummary {
    pub median: usize,
}

impl CountSummary {
    pub fn from_samples(samples: &mut [usize]) -> BenchResult<Self> {
        if samples.is_empty() {
            return Err(io::Error::other("benchmark produced no allocation samples").into());
        }
        samples.sort_unstable();
        Ok(Self {
            median: samples[samples.len() / 2],
        })
    }
}

pub fn read_positive_usize(name: &str, default: usize) -> BenchResult<usize> {
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    let parsed = value.parse::<usize>()?;
    if parsed == 0 {
        return Err(io::Error::other(format!("{name} must be greater than zero")).into());
    }
    Ok(parsed)
}

pub fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
