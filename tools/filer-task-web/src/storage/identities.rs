//! # Identity Storage
//!
//! Persists a stable user behind many browser sessions. Each session is one
//! row in `sessions`, so switching browsers no longer loses the identity and
//! a user can hold several cookies at once. Display names use a
//! Rust-lowercased key so uniqueness follows the same Unicode behavior as the
//! request layer without depending on SQLite's ASCII-only `NOCASE` collation.

use sqlx::{Row, Sqlite, SqliteExecutor, Transaction};

use super::{Storage, StorageError, unix_now};

const LAST_SEEN_WINDOW_SECONDS: i64 = 5 * 60;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSession {
    pub identity: StoredIdentity,
    pub session_id: i64,
    pub last_seen: i64,
    pub is_admin: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_id: i64,
    pub device_label: String,
    pub created_at: i64,
    pub last_seen: i64,
}

impl Storage {
    pub async fn create_identity(
        &self,
        username: &str,
        device_label: &str,
    ) -> Result<IdentitySession, StorageError> {
        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "begin create identity",
                    source,
                })?;
        let identity = insert_user(&mut transaction, username)
            .await?
            .ok_or(StorageError::UsernameTaken)?;
        let session_token =
            insert_session(&mut transaction, identity.user_id, device_label).await?;
        transaction
            .commit()
            .await
            .map_err(|source| StorageError::Operation {
                operation: "commit create identity",
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
    ) -> Result<Option<ResolvedSession>, StorageError> {
        let row = sqlx::query(
            "SELECT s.id AS session_id, s.last_seen AS last_seen, u.id AS user_id, \
                    u.display_name, u.is_admin \
             FROM sessions s JOIN users u ON u.id = s.user_id \
             WHERE s.token = ?",
        )
        .bind(session_token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "resolve identity",
            source,
        })?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut session = decode_resolved(&row, "decode resolved identity")?;
        let now = unix_now()?;
        // The UPDATE is a write transaction in SQLite, so skip it when the row
        // just read already proves last_seen is fresh.
        if session.last_seen < now - LAST_SEEN_WINDOW_SECONDS {
            sqlx::query(
                "UPDATE sessions SET last_seen = unixepoch() \
                 WHERE token = ? AND last_seen < unixepoch() - ?",
            )
            .bind(session_token)
            .bind(LAST_SEEN_WINDOW_SECONDS)
            .execute(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "refresh session last-seen",
                source,
            })?;
            session.last_seen = now;
        }
        Ok(Some(session))
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

    pub async fn mint_recovery_session(
        &self,
        username: &str,
        device_label: &str,
    ) -> Result<IdentitySession, StorageError> {
        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "begin recovery session",
                    source,
                })?;
        // Acquire the write lock before reading so recovery cannot upgrade a stale snapshot.
        let identity =
            match insert_user(&mut transaction, username).await? {
                Some(identity) => identity,
                None => find_user(&mut *transaction, username).await?.ok_or(
                    StorageError::InvalidData {
                        operation: "mint recovery session",
                        message: "username disappeared during creation".to_string(),
                    },
                )?,
            };
        let session_token =
            insert_session(&mut transaction, identity.user_id, device_label).await?;
        transaction
            .commit()
            .await
            .map_err(|source| StorageError::Operation {
                operation: "commit recovery session",
                source,
            })?;
        Ok(IdentitySession {
            identity,
            session_token,
        })
    }

    pub async fn clear_user_sessions(&self, username: &str) -> Result<usize, StorageError> {
        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "begin clear sessions",
                    source,
                })?;
        let Some(identity) = find_user(&mut *transaction, username).await? else {
            return Ok(0);
        };
        let result = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(identity.user_id)
            .execute(&mut *transaction)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "clear user sessions",
                source,
            })?;
        transaction
            .commit()
            .await
            .map_err(|source| StorageError::Operation {
                operation: "commit clear sessions",
                source,
            })?;
        Ok(result.rows_affected() as usize)
    }

    pub async fn list_sessions(&self, user_id: i64) -> Result<Vec<SessionSummary>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, device_label, created_at, last_seen \
             FROM sessions WHERE user_id = ? ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "list sessions",
            source,
        })?;
        rows.iter()
            .map(|row| decode_session_summary(row, "decode listed session"))
            .collect()
    }

    pub async fn revoke_session(
        &self,
        user_id: i64,
        session_id: i64,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = ? AND user_id = ?")
            .bind(session_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "revoke session",
                source,
            })?;
        Ok(result.rows_affected() > 0)
    }

    /// A missing user has no server administration rights.
    pub async fn is_admin(&self, user_id: i64) -> Result<bool, StorageError> {
        let flag: Option<bool> = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "read admin flag",
                source,
            })?;
        Ok(flag.unwrap_or(false))
    }

    pub async fn admin_count(&self) -> Result<i64, StorageError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "count admins",
                source,
            })
    }

    pub async fn user_by_name(
        &self,
        username: &str,
    ) -> Result<Option<StoredIdentity>, StorageError> {
        find_user(&self.pool, username).await
    }

    /// Sets the admin flag for `username`. Returns `None` when no user has
    /// that name.
    pub async fn set_admin(
        &self,
        username: &str,
        is_admin: bool,
    ) -> Result<Option<StoredIdentity>, StorageError> {
        let row = sqlx::query(
            "UPDATE users SET is_admin = ?, updated_at = unixepoch() \
             WHERE name_key = ? RETURNING id, display_name",
        )
        .bind(is_admin)
        .bind(username.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "set admin flag",
            source,
        })?;
        row.map(|row| decode_identity(&row, "decode admin user"))
            .transpose()
    }
}

pub(crate) async fn find_user(
    executor: impl SqliteExecutor<'_>,
    username: &str,
) -> Result<Option<StoredIdentity>, StorageError> {
    let row = sqlx::query("SELECT id, display_name FROM users WHERE name_key = ?")
        .bind(username.to_lowercase())
        .fetch_optional(executor)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "find user",
            source,
        })?;
    row.map(|row| decode_identity(&row, "decode found user"))
        .transpose()
}

pub(crate) async fn insert_user(
    transaction: &mut Transaction<'_, Sqlite>,
    username: &str,
) -> Result<Option<StoredIdentity>, StorageError> {
    // Taking the write lock before checking users gives concurrent first identities one admin.
    let row = sqlx::query(
        "INSERT INTO users (display_name, name_key, is_admin) \
         VALUES (?, ?, NOT EXISTS (SELECT 1 FROM users)) \
         ON CONFLICT(name_key) DO NOTHING \
         RETURNING id, display_name",
    )
    .bind(username)
    .bind(username.to_lowercase())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|source| StorageError::Operation {
        operation: "insert user",
        source,
    })?;
    row.map(|row| decode_identity(&row, "decode inserted user"))
        .transpose()
}

pub(crate) async fn insert_session(
    transaction: &mut Transaction<'_, Sqlite>,
    user_id: i64,
    device_label: &str,
) -> Result<String, StorageError> {
    sqlx::query_scalar(
        "INSERT INTO sessions (user_id, token, device_label) \
         VALUES (?, lower(hex(randomblob(32))), ?) \
         RETURNING token",
    )
    .bind(user_id)
    .bind(device_label)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|source| StorageError::Operation {
        operation: "insert session",
        source,
    })
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

fn decode_session_summary(
    row: &sqlx::sqlite::SqliteRow,
    operation: &'static str,
) -> Result<SessionSummary, StorageError> {
    let session_id = row
        .try_get("id")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let device_label = row
        .try_get("device_label")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let created_at = row
        .try_get("created_at")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let last_seen = row
        .try_get("last_seen")
        .map_err(|source| StorageError::Operation { operation, source })?;
    Ok(SessionSummary {
        session_id,
        device_label,
        created_at,
        last_seen,
    })
}

fn decode_resolved(
    row: &sqlx::sqlite::SqliteRow,
    operation: &'static str,
) -> Result<ResolvedSession, StorageError> {
    let user_id = row
        .try_get("user_id")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let username = row
        .try_get("display_name")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let session_id = row
        .try_get("session_id")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let last_seen = row
        .try_get("last_seen")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let is_admin = row
        .try_get("is_admin")
        .map_err(|source| StorageError::Operation { operation, source })?;
    Ok(ResolvedSession {
        identity: StoredIdentity { user_id, username },
        session_id,
        last_seen,
        is_admin,
    })
}
