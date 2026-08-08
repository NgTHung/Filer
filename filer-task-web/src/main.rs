//! Serves the filer-task web interface on localhost only.
//!
//! Binds `127.0.0.1` so the task board is never exposed to the network. Pass
//! `--port <port>` to change the port and `--database <path>` to select the
//! SQLite file. Project registrations are loaded from that database.
//!
//! Two recovery subcommands help someone who lost every cookie regain their
//! identity without a pairing PIN: `session-mint <username>` prints a fresh
//! session cookie value for that user (creating the user if needed), and
//! `session-clear <username>` revokes that user's sessions and prints how many
//! it removed. Unknown arguments abort with a usage message instead of silently
//! falling back to defaults, so a typo like `--databse` cannot hit the wrong
//! database.

use std::{net::SocketAddr, path::PathBuf};

use axum::http::{HeaderValue, header};
use filer_task_web::{
    app::{AppState, router},
    storage::Storage,
};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = match parse_args() {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };
    let result = match command {
        Command::Serve(options) => serve(options).await,
        Command::SessionMint { username, database } => session_mint(&username, &database).await,
        Command::SessionClear { username, database } => session_clear(&username, &database).await,
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
    Ok(())
}

async fn serve(options: ServeOptions) -> Result<(), Box<dyn std::error::Error>> {
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

async fn session_mint(
    username: &str,
    database: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    let session = storage
        .mint_recovery_session(username, filer_task_web::device_label::RECOVERY_CLI_LABEL)
        .await?;
    println!("{}", session.session_token);
    eprintln!("minted a fresh session for {}", session.identity.username);
    Ok(())
}

async fn session_clear(
    username: &str,
    database: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    let cleared = storage.clear_user_sessions(username).await?;
    println!("{cleared}");
    eprintln!("cleared {cleared} sessions for {username}");
    Ok(())
}

enum Command {
    Serve(ServeOptions),
    SessionMint { username: String, database: PathBuf },
    SessionClear { username: String, database: PathBuf },
}

struct ServeOptions {
    port: u16,
    database: PathBuf,
}

const USAGE: &str = "\
usage:
  filer-task-web [--port <port>] [--database <path>]
  filer-task-web session-mint <username> [--database <path>]
  filer-task-web session-clear <username> [--database <path>]";

fn parse_args() -> Result<Command, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("session-mint") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::SessionMint { username, database }),
        Some("session-clear") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::SessionClear { username, database }),
        _ => parse_serve(&args),
    }
}

fn parse_serve(args: &[String]) -> Result<Command, String> {
    let mut port = 7878;
    let mut database = PathBuf::from("filer-task-web.sqlite3");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--port" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--port requires a value".to_string())?;
                port = value
                    .parse()
                    .map_err(|_| format!("invalid --port value {value:?}"))?;
                index += 2;
            }
            "--database" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--database requires a value".to_string())?;
                database = PathBuf::from(value);
                index += 2;
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    Ok(Command::Serve(ServeOptions { port, database }))
}

fn parse_session_subcommand(args: &[String]) -> Result<(String, PathBuf), String> {
    let mut database = PathBuf::from("filer-task-web.sqlite3");
    let mut username = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--database" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--database requires a value".to_string())?;
                database = PathBuf::from(value);
                index += 2;
            }
            flag if flag.starts_with('-') => return Err(format!("unexpected flag {flag:?}")),
            positional => {
                if username.is_some() {
                    return Err(format!("unexpected argument {positional:?}"));
                }
                username = Some(positional.to_string());
                index += 1;
            }
        }
    }
    let username = username.ok_or_else(|| "missing username".to_string())?;
    Ok((username, database))
}
