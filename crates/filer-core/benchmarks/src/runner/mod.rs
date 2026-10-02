//! # Benchmark runner
//!
//! The runner turns one scenario into one adapter process. It builds the
//! request from trusted plan inputs, streams every stdout line through the
//! validator, and keeps timings only when the whole trace is accepted. Any
//! rejection stops the adapter at once and keeps its raw output and stderr, so
//! a failed sample can be diagnosed without being rerun.
//!
//! Each sample runs in a fresh process because the protocol identifies a
//! sample by its process and a cold start must not inherit earlier work.
//!
//! An adapter is invoked as `<program> <args>... --fixture-root <path>`
//! followed by one `--metric <name>` pair per requested metric. These are
//! trusted inputs outside the wire request.
//!
//! ```no_run
//! # use filer_core_benchmarks::{PreparedFixture, RunPlan, Runner, SampleSpec, ValidatedManifest};
//! # fn example(
//! #     plan: RunPlan,
//! #     manifest: ValidatedManifest,
//! #     fixture: &PreparedFixture,
//! # ) -> Result<(), Box<dyn std::error::Error>> {
//! let mut runner = Runner::new(plan, manifest, fixture)?;
//! let record = runner.run_sample(SampleSpec {
//!     sample_id: "sample-0001".to_string(),
//!     process_id: "process-0001".to_string(),
//!     order_id: "round-01-position-01".to_string(),
//!     scenario_id: "browse.fast.first".to_string(),
//! })?;
//! println!("timing eligible: {}", record.outcome.timing_eligible());
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::fixtures::PreparedFixture;
use crate::manifests::ValidatedManifest;
use crate::profile::ProfileRecord;
use crate::scenarios::ScenarioKind;
use crate::schema::{
    Adapter, CacheState, Clock, Environment, Group, Implementation, Request, StatusKind,
    encode_request_line,
};
use crate::validator::{DeclaredCapabilities, RunValidator, ValidatedSample};
use crate::{ErrorCode, ErrorContext, ProtocolError};

mod process;

use process::{AdapterRun, RunFailure};

/// Version 1 viewport and page sizes; the validator rejects any other value.
const VIEWPORT_SIZE: u64 = 40;
const PAGE_SIZE: u64 = 256;

#[derive(Clone, Debug)]
pub struct AdapterSpec {
    pub identity: Adapter,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub capabilities: DeclaredCapabilities,
}

/// Trusted inputs shared by every sample in one run.
#[derive(Clone, Debug)]
pub struct RunPlan {
    pub run_id: String,
    pub implementation: Implementation,
    pub build: ProfileRecord,
    pub machine: ProfileRecord,
    pub filesystem: ProfileRecord,
    pub adapter: AdapterSpec,
    /// Declared by the caller because the runner must not infer cache state.
    pub cache: CacheState,
    pub requested_metrics: Vec<String>,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleSpec {
    pub sample_id: String,
    pub process_id: String,
    pub order_id: String,
    pub scenario_id: String,
}

#[derive(Clone, Debug)]
pub struct SampleRecord {
    pub outcome: SampleOutcome,
    pub diagnostics: AdapterDiagnostics,
}

#[derive(Clone, Debug)]
pub enum SampleOutcome {
    Accepted(Box<ValidatedSample>),
    Rejected(SampleFailure),
}

impl SampleOutcome {
    /// Only an accepted success may contribute timings to a result set.
    pub fn timing_eligible(&self) -> bool {
        matches!(self, Self::Accepted(sample) if sample.status.kind == StatusKind::Success)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FailureCode {
    Protocol(ErrorCode),
    AdapterSpawnFailed,
    AdapterTimeout,
    AdapterExitStatus,
    AdapterIo,
}

impl FailureCode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Protocol(code) => code.as_str(),
            Self::AdapterSpawnFailed => "adapter_spawn_failed",
            Self::AdapterTimeout => "adapter_timeout",
            Self::AdapterExitStatus => "adapter_exit_status",
            Self::AdapterIo => "adapter_io",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleFailure {
    pub code: FailureCode,
    pub message: String,
    pub context: ErrorContext,
}

impl SampleFailure {
    pub const fn code_str(&self) -> &'static str {
        self.code.as_str()
    }

    fn from_protocol(error: ProtocolError) -> Self {
        Self {
            code: FailureCode::Protocol(error.code()),
            message: error.message().to_string(),
            context: error.context().clone(),
        }
    }
}

/// What the adapter process left behind, independent of validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterDiagnostics {
    /// The platform's description of how the process ended, once reaped.
    pub exit_status: Option<String>,
    pub exit_code: Option<i32>,
    pub stderr: String,
    pub stderr_truncated: bool,
    /// Raw stdout lines, kept only for rejected samples.
    pub stdout_lines: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub enum RunnerError {
    Plan(String),
    Request(ProtocolError),
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(message) => write!(formatter, "invalid run plan: {message}"),
            Self::Request(error) => write!(formatter, "request rejected: {error}"),
        }
    }
}

impl std::error::Error for RunnerError {}

pub struct Runner<'fixture> {
    plan: RunPlan,
    fixture: &'fixture PreparedFixture,
    validator: RunValidator,
}

impl<'fixture> Runner<'fixture> {
    pub fn new(
        plan: RunPlan,
        manifest: ValidatedManifest,
        fixture: &'fixture PreparedFixture,
    ) -> Result<Self, RunnerError> {
        if fixture.manifest_id() != manifest.id()
            || fixture.manifest_digest() != manifest.manifest_digest()
        {
            return Err(RunnerError::Plan(
                "prepared fixture does not belong to the selected manifest".to_string(),
            ));
        }
        let validator = RunValidator::new(
            manifest,
            plan.adapter.capabilities.clone(),
            plan.requested_metrics.iter().cloned(),
        )
        .map_err(RunnerError::Request)?;
        Ok(Self {
            plan,
            fixture,
            validator,
        })
    }

    /// Run one sample in a new adapter process.
    ///
    /// Adapter failures produce a rejected record. An error means the runner
    /// itself could not form the request.
    pub fn run_sample(&mut self, spec: SampleSpec) -> Result<SampleRecord, RunnerError> {
        let request = self.request(&spec)?;
        let request_line = encode_request_line(&request);
        let command = self.command();
        let mut sample = self
            .validator
            .start_sample(&request_line)
            .map_err(RunnerError::Request)?;
        let run = process::run_adapter(command, &request_line, self.plan.timeout, |line| {
            sample.ingest_line(line)
        });
        let AdapterRun {
            failure,
            mut diagnostics,
            exited_successfully,
        } = run;
        let outcome = match failure {
            Some(RunFailure::Protocol(error)) => {
                SampleOutcome::Rejected(SampleFailure::from_protocol(error))
            }
            Some(RunFailure::Adapter(failure)) => SampleOutcome::Rejected(failure),
            None => match sample.finish() {
                Err(error) => SampleOutcome::Rejected(SampleFailure::from_protocol(error)),
                Ok(_) if !exited_successfully => SampleOutcome::Rejected(SampleFailure {
                    code: FailureCode::AdapterExitStatus,
                    message: "adapter exited unsuccessfully after a valid trace".to_string(),
                    context: ErrorContext::default(),
                }),
                Ok(validated) => SampleOutcome::Accepted(Box::new(validated)),
            },
        };
        if matches!(outcome, SampleOutcome::Accepted(_)) {
            diagnostics.stdout_lines.clear();
        }
        Ok(SampleRecord {
            outcome,
            diagnostics,
        })
    }

    fn request(&self, spec: &SampleSpec) -> Result<Request, RunnerError> {
        let settings = ScenarioKind::from_id(&spec.scenario_id)
            .ok_or_else(|| {
                RunnerError::Request(ProtocolError::new(
                    ErrorCode::InvalidScenarioConfiguration,
                    "scenario_id is not a version 1 scenario",
                ))
            })?
            .request_settings();
        let plan = &self.plan;
        Ok(Request {
            protocol_version: 1,
            message_type: "run_request".to_string(),
            run_id: plan.run_id.clone(),
            sample_id: spec.sample_id.clone(),
            process_id: spec.process_id.clone(),
            order_id: spec.order_id.clone(),
            scenario_id: spec.scenario_id.clone(),
            fixture: self.validator.manifest().fixture_reference(),
            implementation: plan.implementation.clone(),
            adapter: plan.adapter.identity.clone(),
            environment: Environment {
                machine_profile_id: plan.machine.id().to_string(),
                machine_profile_digest: plan.machine.digest(),
                filesystem_profile_id: plan.filesystem.id().to_string(),
                filesystem_profile_digest: plan.filesystem.digest(),
            },
            cache: plan.cache.clone(),
            viewport_size: VIEWPORT_SIZE,
            page_size: PAGE_SIZE,
            requested_fields: settings.requested_fields,
            sort: settings.sort,
            filter: settings.filter,
            group: Group::None,
            search: Group::None,
            clock: Clock::ProcessMonotonicNanosecond,
        })
    }

    fn command(&self) -> Command {
        let adapter = &self.plan.adapter;
        let mut command = Command::new(&adapter.program);
        command
            .args(&adapter.args)
            .arg("--fixture-root")
            .arg(self.fixture.root());
        for metric in &self.plan.requested_metrics {
            command.arg("--metric").arg(metric);
        }
        command
    }
}
