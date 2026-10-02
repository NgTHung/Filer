//! # Adapter process lifecycle
//!
//! This module owns one adapter child from spawn to reap. Stdout lines go to
//! the caller as they arrive so a rejection can stop the adapter immediately
//! instead of waiting for a misbehaving process to finish. Every exit path
//! kills and waits for the child, so no adapter outlives its sample.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{AdapterDiagnostics, FailureCode, SampleFailure};
use crate::{ErrorContext, ProtocolError};

/// Bounds diagnostic memory while still draining stderr so the child never blocks.
const STDERR_LIMIT: usize = 1024 * 1024;
const EXIT_POLL: Duration = Duration::from_millis(5);

pub(super) enum RunFailure {
    Protocol(ProtocolError),
    Adapter(SampleFailure),
}

pub(super) struct AdapterRun {
    pub(super) failure: Option<RunFailure>,
    pub(super) diagnostics: AdapterDiagnostics,
    pub(super) exited_successfully: bool,
}

pub(super) fn run_adapter(
    mut command: Command,
    request: &[u8],
    timeout: Duration,
    mut on_line: impl FnMut(&[u8]) -> Result<(), ProtocolError>,
) -> AdapterRun {
    let deadline = Instant::now() + timeout;
    let mut diagnostics = AdapterDiagnostics::default();
    let spawned = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            return AdapterRun {
                failure: Some(adapter_failure(
                    FailureCode::AdapterSpawnFailed,
                    format!("cannot start adapter: {error}"),
                )),
                diagnostics,
                exited_successfully: false,
            };
        }
    };
    let stderr = child.stderr.take().map(spawn_stderr_reader);
    let (lines, stdout) = match child.stdout.take() {
        Some(stdout) => {
            let (sender, receiver) = mpsc::channel();
            (Some(receiver), Some(spawn_stdout_reader(stdout, sender)))
        }
        None => (None, None),
    };
    // A closed stdin is the adapter's request EOF, so it is dropped after writing.
    let mut failure = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(request).err().map(|error| {
            adapter_failure(
                FailureCode::AdapterIo,
                format!("cannot write request: {error}"),
            )
        }),
        None => Some(adapter_failure(
            FailureCode::AdapterIo,
            "adapter stdin is unavailable",
        )),
    };
    if failure.is_none()
        && let Some(lines) = &lines
    {
        failure = consume_lines(lines, deadline, &mut diagnostics, &mut on_line);
    }
    drop(lines);
    let status = if failure.is_some() {
        stop(&mut child)
    } else if let Some(status) = wait_until(&mut child, deadline) {
        status
    } else {
        failure = Some(timeout_failure());
        stop(&mut child)
    };
    let reaped = status.as_ref().ok().copied();
    if let Err(error) = &status {
        failure.get_or_insert_with(|| {
            adapter_failure(
                FailureCode::AdapterIo,
                format!("cannot wait for adapter: {error}"),
            )
        });
    }
    if let Some(reader) = stdout
        && reader.join().is_err()
    {
        failure.get_or_insert_with(|| {
            adapter_failure(FailureCode::AdapterIo, "stdout reader panicked")
        });
    }
    if let Some(reader) = stderr {
        match reader.join() {
            Ok((bytes, truncated)) => {
                diagnostics.stderr = String::from_utf8_lossy(&bytes).into_owned();
                diagnostics.stderr_truncated = truncated;
            }
            Err(_) => diagnostics.stderr = "stderr reader panicked".to_string(),
        }
    }
    diagnostics.exit_status = reaped.map(|status| status.to_string());
    diagnostics.exit_code = reaped.and_then(|status| status.code());
    AdapterRun {
        failure,
        diagnostics,
        exited_successfully: reaped.is_some_and(|status| status.success()),
    }
}

fn consume_lines(
    lines: &mpsc::Receiver<io::Result<Vec<u8>>>,
    deadline: Instant,
    diagnostics: &mut AdapterDiagnostics,
    on_line: &mut impl FnMut(&[u8]) -> Result<(), ProtocolError>,
) -> Option<RunFailure> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(remaining) {
            Ok(Ok(line)) => {
                let result = on_line(&line);
                diagnostics.stdout_lines.push(line);
                if let Err(error) = result {
                    return Some(RunFailure::Protocol(error));
                }
            }
            Ok(Err(error)) => {
                return Some(adapter_failure(
                    FailureCode::AdapterIo,
                    format!("cannot read adapter stdout: {error}"),
                ));
            }
            Err(RecvTimeoutError::Timeout) => return Some(timeout_failure()),
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

fn spawn_stdout_reader(
    stdout: impl Read + Send + 'static,
    sender: mpsc::Sender<io::Result<Vec<u8>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            let result = match reader.read_until(b'\n', &mut line) {
                Ok(0) => return,
                Ok(_) => Ok(line),
                Err(error) => Err(error),
            };
            let failed = result.is_err();
            // A closed receiver means the runner already stopped this sample.
            if sender.send(result).is_err() || failed {
                return;
            }
        }
    })
}

fn spawn_stderr_reader(stderr: impl Read + Send + 'static) -> JoinHandle<(Vec<u8>, bool)> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut limited = stderr.take(STDERR_LIMIT as u64 + 1);
        let read = limited.read_to_end(&mut bytes);
        let truncated = bytes.len() > STDERR_LIMIT;
        bytes.truncate(STDERR_LIMIT);
        if read.is_ok() {
            // Drain the rest so a chatty adapter cannot block on a full pipe.
            let _ = io::copy(&mut limited.into_inner(), &mut io::sink());
        }
        (bytes, truncated)
    })
}

/// Waits for a voluntary exit until the deadline; `None` means it is still running.
fn wait_until(child: &mut Child, deadline: Instant) -> Option<io::Result<ExitStatus>> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(Ok(status)),
            Ok(None) if Instant::now() >= deadline => return None,
            Ok(None) => thread::sleep(EXIT_POLL),
            Err(error) => return Some(Err(error)),
        }
    }
}

fn stop(child: &mut Child) -> io::Result<ExitStatus> {
    // Kill fails only when the child already exited, which wait then reports.
    let _ = child.kill();
    child.wait()
}

fn timeout_failure() -> RunFailure {
    adapter_failure(
        FailureCode::AdapterTimeout,
        "adapter did not finish before the sample deadline",
    )
}

fn adapter_failure(code: FailureCode, message: impl Into<String>) -> RunFailure {
    RunFailure::Adapter(SampleFailure {
        code,
        message: message.into(),
        context: ErrorContext::default(),
    })
}
