//! # Filer public-command adapter
//!
//! This adapter measures Filer-core through the same `Command` and `Event`
//! contract a client uses, with no private hooks. It reads one protocol
//! request on stdin, starts the default core and a session before
//! `sample.started`, runs the scenario's actions, and writes the trace after
//! the core has shut down so encoding and teardown stay out of every timing.
//!
//! Supported scenarios are `browse.fast.first`, `browse.fast.scale`,
//! `browse.metadata.first`, and `browse.next`. Others report `not_supported`.
//! Besides `cpu_time_ns` and `peak_rss_bytes`, it reports `core_event_count`:
//! the public core events received during the measured actions.

mod scenarios;
mod session;

use std::io::{self, Read, Write};
use std::process::ExitCode;

use filer_core_benchmarks::{
    AdapterArgs, AdapterTrace, Event, Field, MetricValue, Request, ResourceMeter, Status,
    StatusKind, UnavailableReason, encode_event_line, parse_request_bytes, requested_metric_values,
};

use session::{AdapterFailure, CoreSession};

const CORE_EVENT_COUNT: &str = "core_event_count";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("filer-public-adapter: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let args = AdapterArgs::parse(std::env::args_os().skip(1))?;
    let mut bytes = Vec::new();
    io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read request: {error}"))?;
    let request = parse_request_bytes(&bytes).map_err(|error| error.to_string())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))?;
    let (events, shutdown) = runtime.block_on(sample(&args, request));
    let mut stdout = io::stdout().lock();
    for event in &events {
        stdout
            .write_all(&encode_event_line(event))
            .map_err(|error| format!("cannot write trace: {error}"))?;
    }
    stdout
        .flush()
        .map_err(|error| format!("cannot write trace: {error}"))?;
    // A trace is only trustworthy if the core stopped all work afterwards.
    match shutdown {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(failure) => {
            eprintln!(
                "filer-public-adapter: {}: {}",
                failure.code, failure.message
            );
            Ok(ExitCode::from(1))
        }
    }
}

async fn sample(args: &AdapterArgs, request: Request) -> (Vec<Event>, Result<(), AdapterFailure>) {
    let scenario_id = request.scenario_id.clone();
    let metadata = request.requested_fields.contains(&Field::SizeBytes);
    let opened = CoreSession::open(&args.fixture_root, metadata).await;
    let mut trace = AdapterTrace::new(request);
    let meter = ResourceMeter::start();
    trace.sample_started();
    let (result, session) = match opened {
        Ok(mut session) if scenarios::SUPPORTED.contains(&scenario_id.as_str()) => {
            let result = scenarios::run(&scenario_id, &mut trace, &mut session).await;
            (Some(result), Some(session))
        }
        Ok(session) => (None, Some(session)),
        Err(failure) => (Some(Err(failure)), None),
    };
    let resources = meter.finish();
    let core_events = session.as_ref().map(CoreSession::received_events);
    let metrics = requested_metric_values(&args.metrics, |name| match name {
        CORE_EVENT_COUNT => Some(core_events.map_or(
            MetricValue::Unavailable(UnavailableReason::NotObservable),
            MetricValue::Observed,
        )),
        _ => resources.get(name),
    });
    let status = match result {
        None => Status {
            kind: StatusKind::NotSupported,
            code: Some("scenario_not_supported".to_string()),
            message: Some(format!("{scenario_id} is not supported by filer-public")),
        },
        Some(Ok(())) => Status {
            kind: StatusKind::Success,
            code: None,
            message: None,
        },
        Some(Err(failure)) => Status {
            kind: StatusKind::Error,
            code: Some(failure.code.to_string()),
            message: Some(failure.message),
        },
    };
    let events = trace.finish(status, metrics);
    let shutdown = match session {
        Some(session) => session.close().await,
        None => Ok(()),
    };
    (events, shutdown)
}
