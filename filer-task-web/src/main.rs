//! Serves the filer-task web interface on localhost only.
//!
//! Binds `127.0.0.1` so the task board is never exposed to the network. Pass
//! `--root <path>` to pick the repo (defaults to the working directory) and
//! `--port <port>` to change the port.

use std::{net::SocketAddr, path::PathBuf};

use filer_task_web::app::{AppState, router};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::from_args();

    let state = match AppState::single(options.root) {
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
    root: PathBuf,
    port: u16,
}

impl Options {
    fn from_args() -> Self {
        let mut root = PathBuf::from(".");
        let mut port = 7878;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--root" => {
                    if let Some(value) = args.next() {
                        root = PathBuf::from(value);
                    }
                }
                "--port" => {
                    if let Some(value) = args.next().and_then(|raw| raw.parse().ok()) {
                        port = value;
                    }
                }
                _ => {}
            }
        }
        Self { root, port }
    }
}
