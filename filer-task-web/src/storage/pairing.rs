//! # Pairing PIN Storage
//!
//! Lets an already-authenticated browser hand its identity to a new browser
//! through a short-lived, single-use PIN. A six-digit PIN is safe here because
//! its strength comes from its bounds, not its length: it expires in five
//! minutes, burns on its first successful redemption, and dies after five
//! failed attempts. Redemption asks for the username as well as the PIN so a
//! mistyped PIN cannot silently land someone in another person's identity.
//!
//! Failed attempts count twice. One counter lives on the PIN row, so mistyping
//! a real PIN locks that code; the other lives on the user, so guessing PIN
//! values that do not exist locks that username. Both reset when a redemption
//! succeeds or the five-minute window passes, which keeps a correct code
//! usable after a lockout and bounds a targeted brute force against a known
//! username instead of only against known codes.

use rand::random_range;
use sqlx::Row;

use super::{Storage, StorageError, unix_now};
use crate::storage::identities::{StoredIdentity, insert_session};

const PIN_TTL_SECONDS: i64 = 5 * 60;
const PIN_MAX_ATTEMPTS: i64 = 5;
const PIN_GENERATION_ATTEMPTS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingPin {
    pub pin: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedeemOutcome {
    Session(super::IdentitySession),
    UnknownUsername,
    WrongPin,
    Expired,
    Consumed,
    Locked,
}

impl Storage {
    pub async fn mint_pairing_pin(
        &self,
        user_id: i64,
        session_id: i64,
    ) -> Result<PairingPin, StorageError> {
        // Dead PINs keep their globally unique values reserved forever, so each
        // mint drops them first to free the value space.
        sqlx::query(
            "DELETE FROM pairing_pins \
             WHERE consumed_at IS NOT NULL OR expires_at <= unixepoch()",
        )
        .execute(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "purge dead pairing pins",
            source,
        })?;
        sqlx::query(
            "DELETE FROM pairing_pins \
             WHERE user_id = ? AND consumed_at IS NULL AND expires_at > unixepoch()",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "replace live pairing pins",
            source,
        })?;
        let expires_at = unix_now()? + PIN_TTL_SECONDS;
        for _ in 0..PIN_GENERATION_ATTEMPTS {
            let pin = format!("{:06}", random_range(0..1_000_000u32));
            let inserted = sqlx::query(
                "INSERT INTO pairing_pins (user_id, session_id, pin, expires_at) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(user_id)
            .bind(session_id)
            .bind(&pin)
            .bind(expires_at)
            .execute(&self.pool)
            .await;
            match inserted {
                Ok(_) => return Ok(PairingPin { pin, expires_at }),
                Err(source)
                    if source
                        .as_database_error()
                        .is_some_and(|error| error.is_unique_violation()) =>
                {
                    continue;
                }
                Err(source) => {
                    return Err(StorageError::Operation {
                        operation: "mint pairing pin",
                        source,
                    });
                }
            }
        }
        Err(StorageError::InvalidData {
            operation: "mint pairing pin",
            message: "could not generate a unique pin".to_string(),
        })
    }

    pub async fn redeem_pairing_pin(
        &self,
        username: &str,
        pin: &str,
    ) -> Result<RedeemOutcome, StorageError> {
        let user = sqlx::query("SELECT id, display_name FROM users WHERE name_key = ?")
            .bind(username.to_lowercase())
            .fetch_optional(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "find pairing user",
                source,
            })?;
        let Some(user) = user else {
            return Ok(RedeemOutcome::UnknownUsername);
        };
        let identity = decode_pairing_identity(&user, "decode pairing user")?;

        let row = sqlx::query(
            "SELECT user_id, expires_at, consumed_at, attempts \
             FROM pairing_pins WHERE pin = ?",
        )
        .bind(pin)
        .fetch_optional(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "find pairing pin",
            source,
        })?;
        let Some(row) = row else {
            // A nonexistent PIN is still a guess against the username, so it
            // counts toward that user's lockout.
            return self.record_failed_attempt(identity.user_id).await;
        };
        let (pin_user_id, expires_at, consumed_at, attempts) = decode_pin_state(&row)?;

        if consumed_at.is_some() {
            return Ok(RedeemOutcome::Consumed);
        }
        if expires_at < unix_now()? {
            return Ok(RedeemOutcome::Expired);
        }
        if attempts >= PIN_MAX_ATTEMPTS {
            return Ok(RedeemOutcome::Locked);
        }
        if pin_user_id != identity.user_id {
            sqlx::query("UPDATE pairing_pins SET attempts = attempts + 1 WHERE pin = ?")
                .bind(pin)
                .execute(&self.pool)
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "count failed pairing attempt",
                    source,
                })?;
            return self.record_failed_attempt(identity.user_id).await;
        }

        let mut transaction =
            self.pool
                .begin()
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "begin pairing session",
                    source,
                })?;
        // The guarded UPDATE, not the earlier read, decides who consumes the
        // code: concurrent redemptions both read an unconsumed PIN, but only
        // one UPDATE affects a row, so the other resolves to Consumed.
        let consumed = sqlx::query(
            "UPDATE pairing_pins SET consumed_at = unixepoch() \
             WHERE pin = ? AND consumed_at IS NULL",
        )
        .bind(pin)
        .execute(&mut *transaction)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "consume pairing pin",
            source,
        })?;
        if consumed.rows_affected() == 0 {
            transaction
                .rollback()
                .await
                .map_err(|source| StorageError::Operation {
                    operation: "roll back pairing session",
                    source,
                })?;
            return Ok(RedeemOutcome::Consumed);
        }
        let session_token = insert_session(&mut transaction, identity.user_id).await?;
        sqlx::query("DELETE FROM pairing_attempts WHERE user_id = ?")
            .bind(identity.user_id)
            .execute(&mut *transaction)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "reset pairing attempt counter",
                source,
            })?;
        transaction
            .commit()
            .await
            .map_err(|source| StorageError::Operation {
                operation: "commit pairing session",
                source,
            })?;
        Ok(RedeemOutcome::Session(super::IdentitySession {
            identity,
            session_token,
        }))
    }

    async fn record_failed_attempt(&self, user_id: i64) -> Result<RedeemOutcome, StorageError> {
        // Post-state decides the outcome so a correct streak of five failures
        // yields five WrongPin results and the sixth turns Locked, with the
        // count and its window updated atomically in one statement.
        let attempts: i64 = sqlx::query_scalar(
            "INSERT INTO pairing_attempts (user_id, attempts, updated_at) \
             VALUES (?, 1, unixepoch()) \
             ON CONFLICT(user_id) DO UPDATE SET \
             attempts = CASE WHEN pairing_attempts.updated_at < unixepoch() - ? \
                             THEN 1 ELSE pairing_attempts.attempts + 1 END, \
             updated_at = unixepoch() \
             RETURNING attempts",
        )
        .bind(user_id)
        .bind(PIN_TTL_SECONDS)
        .fetch_one(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "count failed pairing attempt",
            source,
        })?;
        Ok(if attempts > PIN_MAX_ATTEMPTS {
            RedeemOutcome::Locked
        } else {
            RedeemOutcome::WrongPin
        })
    }
}

fn decode_pairing_identity(
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

fn decode_pin_state(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<(i64, i64, Option<i64>, i64), StorageError> {
    let operation = "decode pairing pin state";
    let user_id = row
        .try_get("user_id")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let expires_at = row
        .try_get("expires_at")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let consumed_at = row
        .try_get("consumed_at")
        .map_err(|source| StorageError::Operation { operation, source })?;
    let attempts = row
        .try_get("attempts")
        .map_err(|source| StorageError::Operation { operation, source })?;
    Ok((user_id, expires_at, consumed_at, attempts))
}
