//! Runs the browser-module unit tests under `tests/js` through Node's built-in
//! test runner so `cargo test` stays the single entry point for this crate. The
//! suite covers the pure modules only, which need no DOM and no dependencies.

use std::{path::Path, process::Command};

#[test]
fn frontend_modules_pass_their_unit_tests() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = match Command::new(node_binary())
        .arg("--test")
        .arg("tests/js/*.test.js")
        .current_dir(crate_dir)
        .output()
    {
        Ok(output) => output,
        // Node is a frontend-only prerequisite, so a Rust-only machine skips
        // instead of failing a suite it cannot run.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping frontend module tests: node not found on PATH");
            return;
        }
        Err(error) => panic!("node test runner failed to start: {error}"),
    };

    assert!(
        output.status.success(),
        "frontend module tests failed\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn node_binary() -> &'static str {
    // Windows resolves the launcher only with its extension when spawned directly.
    if cfg!(windows) { "node.exe" } else { "node" }
}
