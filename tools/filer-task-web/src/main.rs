//! Serves the filer-task web interface.
//!
//! Binds `127.0.0.1` by default so the task board stays off the network unless
//! `--host <address>` names another interface. Pass `--port <port>` to change
//! the port and `--database <path>` to select the SQLite file. Project
//! registrations are loaded from that database.
//!
//! Two recovery subcommands help someone who lost every cookie regain their
//! identity without a pairing PIN: `session-mint <username>` prints a fresh
//! session cookie value for that user (creating the user if needed), and
//! `session-clear <username>` revokes that user's sessions and prints how many
//! it removed. Unknown arguments abort with a usage message instead of silently
//! falling back to defaults, so a typo like `--databse` cannot hit the wrong
//! database.
//!
//! Membership subcommands are the operator override for project roles:
//! `admin-grant` and `admin-revoke` set the server admin flag, `member-grant`
//! and `member-revoke` change a user's role on a project, and `member-list`
//! prints a project's members and pending requests. They may remove the last
//! owner or admin, and warn when they do.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use axum::http::{HeaderValue, header};
use filer_task_web::{
    access::exposure_warning,
    app::{AppState, router},
    storage::{Role, Storage},
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
        Command::AdminGrant { username, database } => admin_set(&username, &database, true).await,
        Command::AdminRevoke { username, database } => admin_set(&username, &database, false).await,
        Command::MemberGrant {
            project,
            username,
            role,
            database,
        } => member_grant(&project, &username, role, &database).await,
        Command::MemberRevoke {
            project,
            username,
            database,
        } => member_revoke(&project, &username, &database).await,
        Command::MemberList { project, database } => member_list(&project, &database).await,
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

    let addr = SocketAddr::new(options.host, options.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    if let Some(warning) = exposure_warning(options.host) {
        eprintln!("{warning}");
    }
    // Port 0 lets the OS pick a free port, so announce the address actually bound.
    println!(
        "filer-task-web listening on http://{}",
        listener.local_addr()?
    );
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

async fn admin_set(
    username: &str,
    database: &PathBuf,
    is_admin: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    let identity = storage
        .set_admin(username, is_admin)
        .await?
        .ok_or_else(|| format!("no user named {username:?}"))?;
    if is_admin {
        println!("granted admin to {}", identity.username);
    } else {
        println!("revoked admin from {}", identity.username);
        if storage.admin_count().await? == 0 {
            eprintln!(
                "warning: no admins remain, so nobody can register projects until admin-grant runs again"
            );
        }
    }
    Ok(())
}

async fn member_grant(
    project: &str,
    username: &str,
    role: Role,
    database: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    let user = storage
        .user_by_name(username)
        .await?
        .ok_or_else(|| format!("no user named {username:?}"))?;
    storage
        .grant_member(project, user.user_id, role, None, false)
        .await?;
    println!("{} is {role} of {project}", user.username);
    warn_if_ownerless(&storage, project).await
}

async fn member_revoke(
    project: &str,
    username: &str,
    database: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    ensure_registered(&storage, project).await?;
    let user = storage
        .user_by_name(username)
        .await?
        .ok_or_else(|| format!("no user named {username:?}"))?;
    match storage.remove_member(project, user.user_id, false).await? {
        Some(removed) => println!(
            "removed {} ({}) from {project}",
            removed.username, removed.role
        ),
        None => println!("{} was not a member of {project}", user.username),
    }
    warn_if_ownerless(&storage, project).await
}

async fn member_list(project: &str, database: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::open(database).await?;
    ensure_registered(&storage, project).await?;
    for member in storage.list_members(project).await? {
        println!("{}\t{}", member.role, member.username);
    }
    for request in storage.list_access_requests(project).await? {
        println!("pending\t{}", request.username);
    }
    Ok(())
}

async fn ensure_registered(
    storage: &Storage,
    project: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let registered = storage
        .project_registrations()
        .await?
        .iter()
        .any(|registration| registration.name == project);
    if registered {
        Ok(())
    } else {
        Err(format!("project {project} is not registered").into())
    }
}

async fn warn_if_ownerless(
    storage: &Storage,
    project: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let has_owner = storage
        .list_members(project)
        .await?
        .iter()
        .any(|member| member.role == Role::Owner);
    if !has_owner {
        eprintln!(
            "warning: {project} has no owner; run member-grant {project} <username> --role owner to assign one"
        );
    }
    Ok(())
}

enum Command {
    Serve(ServeOptions),
    SessionMint {
        username: String,
        database: PathBuf,
    },
    SessionClear {
        username: String,
        database: PathBuf,
    },
    AdminGrant {
        username: String,
        database: PathBuf,
    },
    AdminRevoke {
        username: String,
        database: PathBuf,
    },
    MemberGrant {
        project: String,
        username: String,
        role: Role,
        database: PathBuf,
    },
    MemberRevoke {
        project: String,
        username: String,
        database: PathBuf,
    },
    MemberList {
        project: String,
        database: PathBuf,
    },
}

struct ServeOptions {
    host: IpAddr,
    port: u16,
    database: PathBuf,
}

const DEFAULT_DATABASE: &str = "filer-task-web.sqlite3";

const USAGE: &str = "\
usage:
  filer-task-web [--host <address>] [--port <port>] [--database <path>]
  filer-task-web session-mint <username> [--database <path>]
  filer-task-web session-clear <username> [--database <path>]
  filer-task-web admin-grant <username> [--database <path>]
  filer-task-web admin-revoke <username> [--database <path>]
  filer-task-web member-grant <project> <username> --role owner|editor [--database <path>]
  filer-task-web member-revoke <project> <username> [--database <path>]
  filer-task-web member-list <project> [--database <path>]";

fn parse_args() -> Result<Command, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("session-mint") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::SessionMint { username, database }),
        Some("session-clear") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::SessionClear { username, database }),
        Some("admin-grant") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::AdminGrant { username, database }),
        Some("admin-revoke") => parse_session_subcommand(&args[1..])
            .map(|(username, database)| Command::AdminRevoke { username, database }),
        Some("member-grant") => parse_member_subcommand(&args[1..], &["project", "username"], true)
            .and_then(|parsed| {
                let role = parsed
                    .role
                    .ok_or_else(|| "--role is required".to_string())?;
                let [project, username] = <[String; 2]>::try_from(parsed.positionals)
                    .map_err(|_| "expected a project and a username".to_string())?;
                Ok(Command::MemberGrant {
                    project,
                    username,
                    role,
                    database: parsed.database,
                })
            }),
        Some("member-revoke") => {
            parse_member_subcommand(&args[1..], &["project", "username"], false).and_then(
                |parsed| {
                    let [project, username] = <[String; 2]>::try_from(parsed.positionals)
                        .map_err(|_| "expected a project and a username".to_string())?;
                    Ok(Command::MemberRevoke {
                        project,
                        username,
                        database: parsed.database,
                    })
                },
            )
        }
        Some("member-list") => {
            parse_member_subcommand(&args[1..], &["project"], false).and_then(|parsed| {
                let [project] = <[String; 1]>::try_from(parsed.positionals)
                    .map_err(|_| "expected a project".to_string())?;
                Ok(Command::MemberList {
                    project,
                    database: parsed.database,
                })
            })
        }
        _ => parse_serve(&args),
    }
}

fn parse_serve(args: &[String]) -> Result<Command, String> {
    let mut host = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut port = 7878;
    let mut database = PathBuf::from(DEFAULT_DATABASE);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--host" => {
                host = parsed_flag_value(args, index, "--host")?;
                index += 2;
            }
            "--port" => {
                port = parsed_flag_value(args, index, "--port")?;
                index += 2;
            }
            "--database" => {
                database = PathBuf::from(flag_value(args, index, "--database")?);
                index += 2;
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    Ok(Command::Serve(ServeOptions {
        host,
        port,
        database,
    }))
}

fn parse_session_subcommand(args: &[String]) -> Result<(String, PathBuf), String> {
    let mut database = PathBuf::from(DEFAULT_DATABASE);
    let mut username = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--database" => {
                database = PathBuf::from(flag_value(args, index, "--database")?);
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

struct MemberArgs {
    positionals: Vec<String>,
    role: Option<Role>,
    database: PathBuf,
}

fn parse_member_subcommand(
    args: &[String],
    names: &[&str],
    accepts_role: bool,
) -> Result<MemberArgs, String> {
    let mut database = PathBuf::from(DEFAULT_DATABASE);
    let mut role = None;
    let mut positionals = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--database" => {
                database = PathBuf::from(flag_value(args, index, "--database")?);
                index += 2;
            }
            "--role" if accepts_role => {
                let value = flag_value(args, index, "--role")?;
                role = Some(Role::parse(value).ok_or_else(|| {
                    format!("invalid --role value {value:?}, expected owner or editor")
                })?);
                index += 2;
            }
            flag if flag.starts_with('-') => return Err(format!("unexpected flag {flag:?}")),
            positional => {
                if positionals.len() == names.len() {
                    return Err(format!("unexpected argument {positional:?}"));
                }
                positionals.push(positional.to_string());
                index += 1;
            }
        }
    }
    if let Some(missing) = names.get(positionals.len()) {
        return Err(format!("missing {missing}"));
    }
    Ok(MemberArgs {
        positionals,
        role,
        database,
    })
}

fn flag_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parsed_flag_value<T: FromStr>(args: &[String], index: usize, flag: &str) -> Result<T, String> {
    let value = flag_value(args, index, flag)?;
    value
        .parse()
        .map_err(|_| format!("invalid {flag} value {value:?}"))
}
