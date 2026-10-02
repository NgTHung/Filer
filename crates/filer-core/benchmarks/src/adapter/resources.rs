//! # Process resource metrics
//!
//! CPU time is the difference between two `getrusage` readings taken around
//! the measured actions, so untimed adapter startup and trace encoding are not
//! charged to the scenario. Peak resident memory is a process lifetime
//! high-water mark and cannot be bounded to an interval, so it includes
//! startup. Platforms without `getrusage` report `platform_unavailable`
//! instead of an estimate.

use crate::schema::{MetricValue, UnavailableReason};

pub const CPU_TIME_NS: &str = "cpu_time_ns";
pub const PEAK_RSS_BYTES: &str = "peak_rss_bytes";

#[derive(Clone, Copy, Debug)]
struct Usage {
    cpu_time_ns: u64,
    peak_rss_bytes: u64,
}

/// Measures process resources between `start` and `finish`.
#[derive(Clone, Debug)]
pub struct ResourceMeter {
    start: Result<Usage, UnavailableReason>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceReport {
    cpu_time_ns: MetricValue,
    peak_rss_bytes: MetricValue,
}

impl ResourceMeter {
    pub fn start() -> Self {
        Self { start: usage() }
    }

    pub fn finish(&self) -> ResourceReport {
        match (self.start, usage()) {
            (Ok(start), Ok(end)) => ResourceReport {
                cpu_time_ns: MetricValue::Observed(
                    end.cpu_time_ns.saturating_sub(start.cpu_time_ns),
                ),
                peak_rss_bytes: MetricValue::Observed(end.peak_rss_bytes),
            },
            (Err(reason), _) | (_, Err(reason)) => ResourceReport {
                cpu_time_ns: MetricValue::Unavailable(reason),
                peak_rss_bytes: MetricValue::Unavailable(reason),
            },
        }
    }
}

impl ResourceReport {
    /// Returns `None` for metric names this meter does not measure.
    pub fn get(&self, name: &str) -> Option<MetricValue> {
        match name {
            CPU_TIME_NS => Some(self.cpu_time_ns.clone()),
            PEAK_RSS_BYTES => Some(self.peak_rss_bytes.clone()),
            _ => None,
        }
    }
}

#[cfg(unix)]
fn usage() -> Result<Usage, UnavailableReason> {
    let mut raw = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage writes a complete rusage into the valid pointer we pass.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, raw.as_mut_ptr()) };
    if result != 0 {
        return Err(UnavailableReason::NotObservable);
    }
    // SAFETY: getrusage returned success, so it initialized the struct.
    let raw = unsafe { raw.assume_init() };
    let cpu_time_ns = timeval_ns(raw.ru_utime).saturating_add(timeval_ns(raw.ru_stime));
    let max_rss = u64::try_from(raw.ru_maxrss).map_err(|_| UnavailableReason::NotObservable)?;
    // macOS reports bytes; Linux and the BSDs report kibibytes.
    let peak_rss_bytes = if cfg!(target_os = "macos") {
        max_rss
    } else {
        max_rss.saturating_mul(1024)
    };
    Ok(Usage {
        cpu_time_ns,
        peak_rss_bytes,
    })
}

#[cfg(unix)]
fn timeval_ns(value: libc::timeval) -> u64 {
    let seconds = u64::try_from(value.tv_sec).unwrap_or(0);
    let micros = u64::try_from(value.tv_usec).unwrap_or(0);
    seconds
        .saturating_mul(1_000_000_000)
        .saturating_add(micros.saturating_mul(1_000))
}

#[cfg(not(unix))]
fn usage() -> Result<Usage, UnavailableReason> {
    Err(UnavailableReason::PlatformUnavailable)
}
