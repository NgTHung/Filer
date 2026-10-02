//! Runner fixtures that drive the trace replay adapter through real processes.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) use filer_core_benchmarks::{
    Adapter, AdapterSpec, CacheState, DeclaredCapabilities, ErrorCode, FilesystemCache,
    Implementation, PreparedFixture, ProcessCache, ProfileRecord, RunPlan, Runner, RunnerError,
    SampleOutcome, SampleRecord, SampleSpec, SemanticCache, ValidatedManifest, prepare_fixture,
};
pub(super) use serde_json::{Value, json};
use tempfile::TempDir;

pub(super) use super::traces::{TraceBuilder, fast_trace, manifest, resequence};

pub(super) const SCENARIO: &str = "browse.fast.first";

pub(super) struct RunnerSetup {
    directory: TempDir,
    pub(super) manifest: ValidatedManifest,
    pub(super) fixture: PreparedFixture,
}

impl RunnerSetup {
    pub(super) fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let manifest = manifest("flat-10k-v1.json");
        let fixture = prepare_fixture(&manifest, directory.path().join("fixture"))
            .expect("fixture should prepare");
        Self {
            directory,
            manifest,
            fixture,
        }
    }

    pub(super) fn results_dir(&self) -> PathBuf {
        self.directory.path().join("results")
    }

    pub(super) fn write_trace(&self, name: &str, lines: &[Vec<u8>]) -> PathBuf {
        let path = self.directory.path().join(format!("{name}.ndjson"));
        fs::write(&path, lines.concat()).expect("trace should be written");
        path
    }

    pub(super) fn plan(
        &self,
        adapter: AdapterSpec,
        metrics: &[&str],
        timeout: Duration,
    ) -> RunPlan {
        RunPlan {
            run_id: "run-local-001".to_string(),
            implementation: Implementation {
                id: "filer-core".to_string(),
                version: "0.3.1".to_string(),
                source_revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
                build_profile: "release".to_string(),
                binary_digest: digest('0'),
            },
            build: profile("filer-core-release", &[("compiler", "rustc 1.90.0")]),
            machine: profile("linux-x86_64-lab-01", &[("os", "linux")]),
            filesystem: profile("ext4-lab-01", &[("filesystem", "ext4")]),
            adapter,
            cache: CacheState {
                process: ProcessCache::Cold,
                filesystem: FilesystemCache::Warm,
                semantic: SemanticCache::Empty,
            },
            requested_metrics: metrics.iter().map(|name| name.to_string()).collect(),
            timeout,
            results_dir: self.results_dir(),
        }
    }

    pub(super) fn runner(
        &self,
        adapter: AdapterSpec,
        metrics: &[&str],
        timeout: Duration,
    ) -> Runner<'_> {
        Runner::new(
            self.plan(adapter, metrics, timeout),
            self.manifest.clone(),
            &self.fixture,
        )
        .expect("runner plan should be valid")
    }

    /// Replays `lines` through a fresh runner whose results live under `name`.
    pub(super) fn replay(
        &self,
        name: &str,
        lines: &[Vec<u8>],
        extra_args: &[&str],
    ) -> SampleRecord {
        let trace = self.write_trace(name, lines);
        let mut plan = self.plan(replay_adapter(&trace, extra_args), &[], LONG_TIMEOUT);
        plan.results_dir = self.results_dir().join(name);
        Runner::new(plan, self.manifest.clone(), &self.fixture)
            .expect("runner plan should be valid")
            .run_sample(sample_spec())
            .expect("runner should complete the sample")
    }
}

pub(super) const LONG_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) fn replay_adapter(trace: &Path, extra_args: &[&str]) -> AdapterSpec {
    let mut args = vec!["--trace".to_string(), trace.display().to_string()];
    args.extend(extra_args.iter().map(|arg| arg.to_string()));
    AdapterSpec {
        identity: Adapter {
            id: "filer-public".to_string(),
            version: "1.0.0".to_string(),
            binary_digest: digest('1'),
        },
        program: PathBuf::from(env!("CARGO_BIN_EXE_filer-bench-replay")),
        args,
        capabilities: DeclaredCapabilities::new([SCENARIO], true, true),
    }
}

pub(super) fn sample_spec() -> SampleSpec {
    SampleSpec {
        sample_id: "sample-0001".to_string(),
        process_id: "process-0001".to_string(),
        order_id: "round-01-position-02".to_string(),
        scenario_id: SCENARIO.to_string(),
    }
}

pub(super) fn valid_trace(setup: &RunnerSetup) -> TraceBuilder {
    fast_trace(&setup.manifest, SCENARIO, false, true)
}

pub(super) fn mutate_phase(
    trace: &mut TraceBuilder,
    phase: &str,
    mutation: impl FnOnce(&mut Value),
) {
    let event = trace
        .events
        .iter_mut()
        .find(|event| event["phase"] == phase)
        .expect("trace contains the phase");
    mutation(event);
}

pub(super) fn raw_record(record: &SampleRecord) -> Value {
    let bytes = fs::read(&record.path).expect("raw record should exist");
    serde_json::from_slice(&bytes).expect("raw record is JSON")
}

pub(super) fn rejection_code(record: &SampleRecord) -> &str {
    match &record.outcome {
        SampleOutcome::Rejected(failure) => failure.code_str(),
        SampleOutcome::Accepted(_) => panic!("sample should be rejected"),
    }
}

fn profile(id: &str, entries: &[(&str, &str)]) -> ProfileRecord {
    ProfileRecord::new(
        id,
        entries
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string())),
    )
    .expect("profile should be valid")
}

fn digest(fill: char) -> String {
    format!("sha256:{}", fill.to_string().repeat(64))
}
