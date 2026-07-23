//! # SQLite Storage
//!
//! Owns the database pool, migrations, and every SQL operation used by the web
//! server. SQLx provides an async pool that fits the Tokio server and embeds
//! migration checksums in the binary. Downstream modules add typed methods here
//! instead of accessing the pool, which keeps schema details behind one API.
//!
//! ```
//! use filer_task_web::storage::Storage;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let directory = tempfile::tempdir()?;
//! let storage = Storage::open(directory.path().join("state.sqlite3")).await?;
//! assert_eq!(storage.schema_version().await?, 4);
//! storage.close().await;
//! # Ok(())
//! # }
//! ```

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};

use sqlx::{
    SqlitePool,
    migrate::{MigrateError, Migrator},
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

mod activity;
mod identities;
mod projects;

pub use activity::{ActivityFilter, ActivityRecord, NewActivity};
pub use identities::{IdentitySession, StoredIdentity};
pub use projects::ProjectRegistration;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone, Debug)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(|source| StorageError::Open {
                path: path.clone(),
                source,
            })?;

        if let Err(source) = MIGRATOR.run(&pool).await {
            pool.close().await;
            return Err(StorageError::Migrate { path, source });
        }

        Ok(Self { pool })
    }

    pub async fn schema_version(&self) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&self.pool)
            .await
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[derive(Debug)]
pub enum StorageError {
    Open {
        path: PathBuf,
        source: sqlx::Error,
    },
    Migrate {
        path: PathBuf,
        source: MigrateError,
    },
    Operation {
        operation: &'static str,
        source: sqlx::Error,
    },
    InvalidData {
        operation: &'static str,
        message: String,
    },
    UsernameTaken,
    IdentityNotFound(i64),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(
                    formatter,
                    "failed to open SQLite database {}: {source}",
                    path.display()
                )
            }
            Self::Migrate { path, source } => {
                write!(
                    formatter,
                    "failed to migrate SQLite database {}: {source}",
                    path.display()
                )
            }
            Self::Operation { operation, source } => {
                write!(formatter, "failed to {operation}: {source}")
            }
            Self::InvalidData { operation, message } => {
                write!(formatter, "failed to {operation}: {message}")
            }
            Self::UsernameTaken => write!(formatter, "username is already in use"),
            Self::IdentityNotFound(user_id) => {
                write!(formatter, "identity {user_id} does not exist")
            }
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. } => Some(source),
            Self::Migrate { source, .. } => Some(source),
            Self::Operation { source, .. } => Some(source),
            Self::InvalidData { .. } | Self::UsernameTaken | Self::IdentityNotFound(_) => None,
        }
    }
}
