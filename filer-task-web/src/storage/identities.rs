//! # Identity Storage
//!
//! Persists a stable user ID behind an opaque session token. Display names use
//! a Rust-lowercased key so uniqueness follows the same Unicode behavior as the
//! request layer without depending on SQLite's ASCII-only `NOCASE` collation.

use sqlx::Row;

use super::{Storage, StorageError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredIdentity {
    pub user_id: i64,
    pub username: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentitySession {
    pub identity: StoredIdentity,
    pub session_token: String,
}

impl Storage {
    pub async fn create_identity(&self, username: &str) -> Result<IdentitySession, StorageError> {
        let row = sqlx::query(
            "INSERT INTO users (session_token, display_name, name_key) \
             VALUES (lower(hex(randomblob(32))), ?, ?) \
             ON CONFLICT(name_key) DO NOTHING \
             RETURNING id, session_token, display_name",
        )
        .bind(username)
        .bind(username.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "create identity",
            source,
        })?;
        let Some(row) = row else {
            return Err(StorageError::UsernameTaken);
        };
        let identity = decode_identity(&row, "decode created identity")?;
        let session_token =
            row.try_get("session_token")
                .map_err(|source| StorageError::Operation {
                    operation: "decode identity session token",
                    source,
                })?;
        Ok(IdentitySession {
            identity,
            session_token,
        })
    }

    pub async fn resolve_identity(
        &self,
        session_token: &str,
    ) -> Result<Option<StoredIdentity>, StorageError> {
        let row = sqlx::query("SELECT id, display_name FROM users WHERE session_token = ?")
            .bind(session_token)
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "resolve identity",
                source,
            })?;
        row.map(|row| decode_identity(&row, "decode resolved identity"))
            .transpose()
    }

    pub async fn rename_identity(
        &self,
        user_id: i64,
        username: &str,
    ) -> Result<StoredIdentity, StorageError> {
        let result = sqlx::query(
            "UPDATE users \
             SET display_name = ?, name_key = ?, updated_at = unixepoch() \
             WHERE id = ? \
             RETURNING id, display_name",
        )
        .bind(username)
        .bind(username.to_lowercase())
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await;
        match result {
            Ok(Some(row)) => decode_identity(&row, "decode renamed identity"),
            Ok(None) => Err(StorageError::IdentityNotFound(user_id)),
            Err(source)
                if source
                    .as_database_error()
                    .is_some_and(|error| error.is_unique_violation()) =>
            {
                Err(StorageError::UsernameTaken)
            }
            Err(source) => Err(StorageError::Operation {
                operation: "rename identity",
                source,
            }),
        }
    }
}

fn decode_identity(
    row: &sqlx::sqlite::SqliteRow,
    operation: &'static str,
) -> Result<StoredIdentity, StorageError> {
    let user_id = row
        .try_get("id")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let username = row
        .try_get("display_name")
        .map_err(|source| StorageError::Operation { operation, source })?;
    Ok(StoredIdentity { user_id, username })
}
