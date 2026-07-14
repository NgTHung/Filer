//! Serves the filer-task web interface on localhost only.
//!
//! Binds `127.0.0.1` so the task board is never exposed to the network. Repeat
//! `--root <path>` to serve several repos (defaults to the working
//! directory), pass `--port <port>` to change the port, and pass `--database
//! <path>` to select the SQLite file.

use std::{net::SocketAddr, path::PathBuf};

use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::from_args();
    let storage = match Storage::open(&options.database).await {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("failed to start: {error}");
            return Err(error.into());
        }
    };

    let state = match AppState::from_roots(options.roots, storage) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("failed to start: {error:?}");
            return Err("could not locate a .tasks repository".into());
        }
    };

    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let app = router(state).fallback_service(ServeDir::new(static_dir));

    let addr = SocketAddr::from(([127, 0, 0, 1], options.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("filer-task-web listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

struct Options {
    roots: Vec<PathBuf>,
    port: u16,
    database: PathBuf,
}

impl Options {
    fn from_args() -> Self {
        let mut roots = Vec::new();
        let mut port = 7878;
        let mut database = PathBuf::from("filer-task-web.sqlite3");
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--root" => {
                    if let Some(value) = args.next() {
                        roots.push(PathBuf::from(value));
                    }
                }
                "--port" => {
                    if let Some(value) = args.next().and_then(|raw| raw.parse().ok()) {
                        port = value;
                    }
                }
                "--database" => {
                    if let Some(value) = args.next() {
                        database = PathBuf::from(value);
                    }
                }
                _ => {}
            }
        }
        if roots.is_empty() {
            roots.push(PathBuf::from("."));
        }
        Self {
            roots,
            port,
            database,
        }
    }
}
