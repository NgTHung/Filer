use std::path::PathBuf;
use std::time::Duration;

use filer_core_benchmarks::{
    Adapter, AdapterSpec, CacheState, DeclaredCapabilities, FilesystemCache, GateResult,
    Implementation, MetricValue, Phase, PreparedFixture, ProcessCache, ProfileRecord, RunPlan,
    Runner, SampleOutcome, SampleRecord, SampleSpec, SemanticCache, StatusKind, UnavailableReason,
    ValidatedManifest, prepare_fixture,
};
use tempfile::TempDir;

const SCENARIOS: [&str; 4] = [
    "browse.fast.first",
    "browse.fast.scale",
    "browse.metadata.first",
    "browse.next",
];

struct Setup {
    directory: TempDir,
    manifest: ValidatedManifest,
    fixture: PreparedFixture,
}

impl Setup {
    fn new(manifest_name: &str) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let manifest = ValidatedManifest::load(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("manifests")
                .join(manifest_name),
        )
        .expect("manifest should validate");
        let fixture = prepare_fixture(&manifest, directory.path().join("fixture"))
            .expect("fixture should prepare");
        Self {
            directory,
            manifest,
            fixture,
        }
    }

    fn run(
        &self,
        scenario: &str,
        cache: CacheState,
        metrics: &[&str],
        timeout: Duration,
    ) -> SampleRecord {
        let plan = RunPlan {
            run_id: "run-filer-adapter".to_string(),
            implementation: Implementation {
                id: "filer-core".to_string(),
                version: "0.3.0".to_string(),
                source_revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
                build_profile: "debug".to_string(),
                binary_digest: digest('0'),
            },
            build: profile("filer-core-debug"),
            machine: profile("test-machine"),
            filesystem: profile("test-filesystem"),
            adapter: AdapterSpec {
                identity: Adapter {
                    id: "filer-public".to_string(),
                    version: "1.0.0".to_string(),
                    binary_digest: digest('1'),
                },
                program: PathBuf::from(env!("CARGO_BIN_EXE_filer-public-adapter")),
                args: Vec::new(),
                capabilities: DeclaredCapabilities::new(SCENARIOS, true, false),
            },
            cache,
            requested_metrics: metrics.iter().map(|name| name.to_string()).collect(),
            timeout,
            results_dir: self.directory.path().join("results").join(scenario),
        };
        Runner::new(plan, self.manifest.clone(), &self.fixture)
            .expect("runner plan should be valid")
            .run_sample(SampleSpec {
                sample_id: "sample-0001".to_string(),
                process_id: "process-0001".to_string(),
                order_id: "round-01-position-01".to_string(),
                scenario_id: scenario.to_string(),
            })
            .expect("runner should complete the sample")
    }
}

fn cold() -> CacheState {
    CacheState {
        process: ProcessCache::Cold,
        filesystem: FilesystemCache::Uncontrolled,
        semantic: SemanticCache::Empty,
    }
}

fn profile(id: &str) -> ProfileRecord {
    ProfileRecord::new(id, [("os".to_string(), std::env::consts::OS.to_string())])
        .expect("profile should be valid")
}

fn digest(fill: char) -> String {
    format!("sha256:{}", fill.to_string().repeat(64))
}

fn accepted(record: &SampleRecord) -> &filer_core_benchmarks::ValidatedSample {
    match &record.outcome {
        SampleOutcome::Accepted(sample) => sample,
        SampleOutcome::Rejected(failure) => panic!(
            "sample rejected with {}: {} ({:?}); stderr: {}",
            failure.code_str(),
            failure.message,
            failure.context,
            record.diagnostics.stderr
        ),
    }
}

const TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn browses_flat_10k_through_public_commands() {
    let setup = Setup::new("flat-10k-v1.json");
    let metrics = ["core_event_count", "cpu_time_ns", "peak_rss_bytes"];

    for scenario in ["browse.fast.first", "browse.metadata.first"] {
        let record = setup.run(scenario, cold(), &metrics, TIMEOUT);
        let sample = accepted(&record);

        assert!(record.outcome.timing_eligible(), "{scenario}");
        assert_eq!(
            sample.structural_gates.first_page_examined,
            GateResult::NotEvaluable,
            "examined rows are not observable through public events"
        );
        let first_page = sample
            .timeline
            .iter()
            .filter(|entry| {
                matches!(
                    entry.phase,
                    Phase::RowFirst | Phase::ViewportCommitted | Phase::PageCommitted
                )
            })
            .collect::<Vec<_>>();
        assert!(first_page.len() >= 2);
        assert!(
            first_page
                .iter()
                .all(|entry| entry.timestamp_ns == first_page[0].timestamp_ns),
            "row.first, viewport, and page share the page arrival time"
        );
        assert_eq!(
            first_page[0].counts.examined,
            MetricValue::Unavailable(UnavailableReason::NotObservable)
        );
        assert!(matches!(
            sample.metrics["core_event_count"],
            MetricValue::Observed(count) if count >= 40
        ));
        assert!(matches!(
            sample.metrics["cpu_time_ns"],
            MetricValue::Observed(_)
        ));
    }
}

#[test]
fn follows_the_continuation_chain_one_action_per_page() {
    let setup = Setup::new("flat-10k-v1.json");

    let record = setup.run("browse.next", cold(), &[], TIMEOUT);
    let sample = accepted(&record);

    let pages = sample
        .timeline
        .iter()
        .filter(|entry| entry.phase == Phase::PageCommitted)
        .collect::<Vec<_>>();
    assert_eq!(pages.len(), 40);
    assert_eq!(pages[39].action_id.as_deref(), Some("page-0040"));
    assert_eq!(
        pages[39].output.as_ref().map(|output| output.row_count),
        Some(16)
    );
}

#[test]
fn completes_the_flat_100k_listing() {
    let setup = Setup::new("flat-100k-v1.json");

    let record = setup.run("browse.fast.scale", cold(), &[], TIMEOUT);
    let sample = accepted(&record);

    let listing = sample
        .timeline
        .iter()
        .find(|entry| entry.phase == Phase::ListingCompleted)
        .expect("listing completed");
    assert_eq!(
        listing.output.as_ref().map(|output| output.row_count),
        Some(100_000)
    );
}

#[test]
fn reports_unsupported_scenarios_and_metrics_without_timings() {
    let setup = Setup::new("flat-10k-v1.json");
    let warm = CacheState {
        process: ProcessCache::Warm,
        filesystem: FilesystemCache::Uncontrolled,
        semantic: SemanticCache::Reused,
    };

    let record = setup.run("view.sort.name", warm, &["allocation_count"], TIMEOUT);
    let sample = accepted(&record);

    assert_eq!(sample.status.kind, StatusKind::NotSupported);
    assert!(!record.outcome.timing_eligible());
    assert_eq!(
        sample.metrics["allocation_count"],
        MetricValue::Unavailable(UnavailableReason::Unsupported)
    );
}

#[test]
fn rejects_a_listing_that_misses_a_fixture_entry() {
    let setup = Setup::new("flat-10k-v1.json");
    std::fs::remove_file(setup.fixture.root().join("file-009999.bin"))
        .expect("fixture entry should be removable");

    let record = setup.run("browse.fast.first", cold(), &[], TIMEOUT);

    match &record.outcome {
        SampleOutcome::Rejected(failure) => {
            assert!(
                ["membership_mismatch", "output_row_count_mismatch"].contains(&failure.code_str()),
                "unexpected rejection {}",
                failure.code_str()
            );
        }
        SampleOutcome::Accepted(_) => panic!("an incomplete listing must be rejected"),
    }
}

#[test]
fn kills_the_filer_adapter_at_the_deadline() {
    let setup = Setup::new("flat-10k-v1.json");

    let record = setup.run("browse.fast.first", cold(), &[], Duration::from_millis(1));

    assert!(matches!(
        &record.outcome,
        SampleOutcome::Rejected(failure) if failure.code_str() == "adapter_timeout"
    ));
    assert!(record.diagnostics.exit_status.is_some());
}
