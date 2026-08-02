//! Serves the filer-task web interface on localhost only.
//!
//! Binds `127.0.0.1` so the task board is never exposed to the network. Pass
//! `--port <port>` to change the port and `--database <path>` to select the
//! SQLite file. Project registrations are loaded from that database.

use std::{net::SocketAddr, path::PathBuf};

use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};
use axum::http::{HeaderValue, header};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

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

    let state = match AppState::load(storage).await {
        Ok(state) => state,
        Err(error) => {
            eprintln!("failed to start: {error:?}");
            return Err("could not load the project registry".into());
        }
    };

    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    // Without an explicit header the browser applies heuristic caching and keeps
    // serving a stale stylesheet or module after the file on disk changes.
    let app = router(state)
        .fallback_service(ServeDir::new(static_dir))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));

    let addr = SocketAddr::from(([127, 0, 0, 1], options.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("filer-task-web listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

struct Options {
    port: u16,
    database: PathBuf,
}

impl Options {
    fn from_args() -> Self {
        let mut port = 7878;
        let mut database = PathBuf::from("filer-task-web.sqlite3");
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
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
        Self { port, database }
    }
}
