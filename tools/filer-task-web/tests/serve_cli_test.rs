//! Exercises the serve command of the `filer-task-web` binary end to end. Each
//! test starts the real server on an OS-assigned port against its own temp
//! database, reads the address it announces, and connects to it.

mod cli;

use std::{
    io::{BufRead, BufReader},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
};

const ANNOUNCEMENT: &str = "filer-task-web listening on http://";

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn serve(args: &[&str]) -> (Server, SocketAddr) {
    let mut child = Command::new(cli::binary())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server process spawns");
    let stdout = child.stdout.take().expect("server stdout is piped");
    let server = Server(child);
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("server stdout is readable");
    let addr = line
        .trim()
        .strip_prefix(ANNOUNCEMENT)
        .unwrap_or_else(|| panic!("expected a listening announcement, got {line:?}"))
        .parse()
        .expect("announced address parses");
    (server, addr)
}

#[test]
fn serve_listens_on_loopback_by_default() {
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3").display().to_string();

    let (_server, addr) = serve(&["--port", "0", "--database", &db]);

    assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_ne!(addr.port(), 0);
    TcpStream::connect(addr).expect("server accepts connections");
}

#[test]
fn serve_listens_on_the_requested_host() {
    // The IPv6 loopback differs from the default yet binds without a firewall
    // prompt, so a host without IPv6 skips instead of failing.
    if TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_err() {
        eprintln!("skipping: this machine cannot bind the IPv6 loopback");
        return;
    }
    let temp = tempfile::tempdir().expect("temp dir created");
    let db = temp.path().join("state.sqlite3").display().to_string();

    let (_server, addr) = serve(&["--host", "::1", "--port", "0", "--database", &db]);

    assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    TcpStream::connect(addr).expect("server accepts connections");
}

#[test]
fn serve_rejects_a_missing_or_invalid_host() {
    let cases: &[(&[&str], &str)] = &[
        (&["--host"], "--host requires a value"),
        (
            &["--host", "not-an-ip"],
            "invalid --host value \"not-an-ip\"",
        ),
    ];
    for (args, message) in cases {
        let output = cli::run(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "expected failure for {args:?}");
        assert!(
            stderr.contains(message),
            "expected {message:?} on stderr for {args:?}, got {stderr}"
        );
    }
}
