//! # Trace replay adapter
//!
//! This adapter replays a recorded NDJSON trace instead of measuring an
//! implementation. Runner conformance tests use it to exercise real process
//! lifecycles, including stderr diagnostics, hangs, and failing exit codes,
//! without depending on Filer-core timing.
//!
//! Usage: `filer-bench-replay --trace <path> [--stderr <text>] [--hang]
//! [--exit-code <n>]`. The runner's `--fixture-root` and `--metric` arguments
//! are accepted and ignored because a replayed trace already contains them.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, thread};

use filer_core_benchmarks::parse_request_bytes;

struct Options {
    trace: PathBuf,
    stderr: Option<String>,
    hang: bool,
    exit_code: u8,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut trace = None;
        let mut stderr = None;
        let mut hang = false;
        let mut exit_code = 0;
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{} requires a value", arg.to_string_lossy()))
            };
            match arg.to_str() {
                Some("--trace") => trace = Some(PathBuf::from(value()?)),
                Some("--stderr") => stderr = Some(value()?.to_string_lossy().into_owned()),
                Some("--hang") => hang = true,
                Some("--exit-code") => {
                    exit_code = value()?
                        .to_string_lossy()
                        .parse()
                        .map_err(|_| "--exit-code requires an integer from 0 to 255")?;
                }
                Some("--fixture-root" | "--metric") => {
                    value()?;
                }
                _ => return Err(format!("unknown argument {}", arg.to_string_lossy())),
            }
        }
        Ok(Self {
            trace: trace.ok_or("--trace is required")?,
            stderr,
            hang,
            exit_code,
        })
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("filer-bench-replay: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let options = Options::parse(env::args_os().skip(1))?;
    let mut request = Vec::new();
    io::stdin()
        .read_to_end(&mut request)
        .map_err(|error| format!("cannot read request: {error}"))?;
    parse_request_bytes(&request).map_err(|error| error.to_string())?;
    let trace = fs::read(&options.trace)
        .map_err(|error| format!("cannot read {}: {error}", options.trace.display()))?;
    // Diagnostics go first so a runner that kills a hanging replay still has them.
    if let Some(text) = &options.stderr {
        eprintln!("{text}");
    }
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&trace)
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("cannot write trace: {error}"))?;
    if options.hang {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
    Ok(ExitCode::from(options.exit_code))
}
