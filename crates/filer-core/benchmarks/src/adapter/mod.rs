//! # Adapter-side support
//!
//! Every measured adapter needs the same protocol plumbing: the runner's
//! trusted arguments, a trace with one process clock and exact sequence
//! numbers, and an entry for every requested metric. Keeping that here lets an
//! adapter contain only the work it measures, so two adapters cannot disagree
//! on what a milestone, a total, or an unavailable metric means.
//!
//! ```
//! use std::ffi::OsString;
//! use filer_core_benchmarks::{AdapterArgs, MetricValue, requested_metric_values};
//!
//! let args = AdapterArgs::parse(
//!     ["--fixture-root", "/tmp/flat-10k", "--metric", "allocation_count"].map(OsString::from),
//! )?;
//! let metrics = requested_metric_values(&args.metrics, |_| None);
//! assert!(matches!(metrics["allocation_count"], MetricValue::Unavailable(_)));
//! # Ok::<(), String>(())
//! ```

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::schema::{MetricValue, UnavailableReason};

mod resources;
mod trace;

pub use resources::{ResourceMeter, ResourceReport};
pub use trace::{AdapterTrace, Milestone};

/// The runner-supplied arguments every adapter accepts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterArgs {
    pub fixture_root: PathBuf,
    pub metrics: Vec<String>,
}

impl AdapterArgs {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut fixture_root = None;
        let mut metrics = Vec::new();
        while let Some(arg) = args.next() {
            let name = arg.to_string_lossy().into_owned();
            let value = args
                .next()
                .ok_or_else(|| format!("{name} requires a value"))?;
            match name.as_str() {
                "--fixture-root" => fixture_root = Some(PathBuf::from(value)),
                "--metric" => metrics.push(value.to_string_lossy().into_owned()),
                _ => return Err(format!("unknown argument {name}")),
            }
        }
        Ok(Self {
            fixture_root: fixture_root.ok_or("--fixture-root is required")?,
            metrics,
        })
    }
}

/// Builds the `sample.completed` metrics so every requested name is present.
///
/// `lookup` returns `None` for names the adapter cannot measure at all; those
/// become `unsupported` rather than a fabricated zero.
pub fn requested_metric_values(
    requested: &[String],
    lookup: impl Fn(&str) -> Option<MetricValue>,
) -> BTreeMap<String, MetricValue> {
    requested
        .iter()
        .map(|name| {
            let value =
                lookup(name).unwrap_or(MetricValue::Unavailable(UnavailableReason::Unsupported));
            (name.clone(), value)
        })
        .collect()
}
