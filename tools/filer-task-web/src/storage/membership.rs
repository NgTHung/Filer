//! # Membership Storage
//!
//! Persists who may change each project and who is waiting for access. Members
//! and pending requests live in separate tables, so a membership check is one
//! primary-key lookup with no status filter. A check that spans rows, such as
//! a project keeping at least one owner, runs inside the transaction that would
//! break it and after that transaction's first write, which holds SQLite's
//! write lock, so two concurrent changes cannot both pass it.
//!
//! ```
//! use filer_task_web::storage::Role;
//! assert!(Role::Owner.satisfies(Role::Editor));
//! assert_eq!(Role::parse("owner"), Some(Role::Owner));
//! ```

use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{Storage, StorageError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Editor,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "editor" => Some(Self::Editor),
            _ => None,
        }
    }

    /// Whether holding this role grants what `required` asks for. An owner can
    /// do everything an editor can.
    pub fn satisfies(self, required: Role) -> bool {
        self == Self::Owner || self == required
    }
}

impl fmt::Display for Role {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A user's standing on one project, as the browser needs it: a role, or a
/// request that is still waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectAccess {
    Owner,
    Editor,
    Pending,
}

impl From<Role> for ProjectAccess {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Editor => Self::Editor,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRecord {
    pub user_id: i64,
    pub username: String,
    pub role: Role,
    pub granted_by: Option<String>,
    pub granted_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessRequestRecord {
    pub user_id: i64,
    pub username: String,
    pub requested_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantOutcome {
    pub username: String,
    pub previous: Option<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedMember {
    pub username: String,
    pub role: Role,
}

impl Storage {
    pub async fn project_role(
        &self,
        project: &str,
        user_id: i64,
    ) -> Result<Option<Role>, StorageError> {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM project_members WHERE project = ? AND user_id = ?",
        )
        .bind(project)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(failed("read project role"))?;
        role.map(|role| decode_role(&role)).transpose()
    }

    pub async fn user_project_access(
        &self,
        user_id: i64,
    ) -> Result<HashMap<String, ProjectAccess>, StorageError> {
        let rows = sqlx::query(
            "SELECT project, role FROM project_members WHERE user_id = ? \
             UNION ALL \
             SELECT project, 'pending' FROM access_requests WHERE user_id = ?",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(failed("list project access"))?;
        let mut access = HashMap::new();
        for row in rows {
            let project: String = row
                .try_get("project")
                .map_err(failed("decode project access"))?;
            let role: String = row
                .try_get("role")
                .map_err(failed("decode project access"))?;
            let value = match role.as_str() {
                "pending" => ProjectAccess::Pending,
                other => decode_role(other)?.into(),
            };
            access.insert(project, value);
        }
        Ok(access)
    }

    pub async fn list_members(&self, project: &str) -> Result<Vec<MemberRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT m.user_id, u.display_name AS username, m.role, \
                    g.display_name AS granted_by, m.granted_at \
             FROM project_members m \
             JOIN users u ON u.id = m.user_id \
             LEFT JOIN users g ON g.id = m.granted_by \
             WHERE m.project = ? \
             ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, u.name_key",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await
        .map_err(failed("list members"))?;
        rows.iter().map(decode_member).collect()
    }

    pub async fn list_access_requests(
        &self,
        project: &str,
    ) -> Result<Vec<AccessRequestRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT r.user_id, u.display_name AS username, r.requested_at \
             FROM access_requests r JOIN users u ON u.id = r.user_id \
             WHERE r.project = ? ORDER BY r.requested_at, r.user_id",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await
        .map_err(failed("list access requests"))?;
        rows.iter()
            .map(|row| {
                Ok(AccessRequestRecord {
                    user_id: row
                        .try_get("user_id")
                        .map_err(failed("decode access request"))?,
                    username: row
                        .try_get("username")
                        .map_err(failed("decode access request"))?,
                    requested_at: row
                        .try_get("requested_at")
                        .map_err(failed("decode access request"))?,
                })
            })
            .collect()
    }

    /// Adds `user_id` to `project` with `role`, or changes an existing member's
    /// role, and deletes that user's pending request. With `protect_last_owner`,
    /// a change that would leave the project without an owner is refused and
    /// rolled back; the CLI passes `false` as the operator override.
    pub async fn grant_member(
        &self,
        project: &str,
        user_id: i64,
        role: Role,
        granted_by: Option<i64>,
        protect_last_owner: bool,
    ) -> Result<GrantOutcome, StorageError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(failed("begin grant member"))?;
        // First statement is a write, so the checks below read under the lock.
        sqlx::query("DELETE FROM access_requests WHERE project = ? AND user_id = ?")
            .bind(project)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(failed("clear access request"))?;
        ensure_registered(&mut transaction, project).await?;
        let username = username_of(&mut transaction, user_id).await?;
        let previous = member_role(&mut transaction, project, user_id).await?;
        sqlx::query(
            "INSERT INTO project_members (project, user_id, role, granted_by) VALUES (?, ?, ?, ?) \
             ON CONFLICT(project, user_id) DO UPDATE \
             SET role = excluded.role, granted_by = excluded.granted_by, granted_at = unixepoch() \
             WHERE project_members.role <> excluded.role",
        )
        .bind(project)
        .bind(user_id)
        .bind(role.as_str())
        .bind(granted_by)
        .execute(&mut *transaction)
        .await
        .map_err(failed("upsert member"))?;
        if protect_last_owner && owner_count(&mut transaction, project).await? == 0 {
            return Err(StorageError::LastOwner);
        }
        transaction
            .commit()
            .await
            .map_err(failed("commit grant member"))?;
        Ok(GrantOutcome { username, previous })
    }

    /// Removes `user_id` from `project`. Returns `None` when they were not a
    /// member. With `protect_last_owner`, removing the last owner is refused.
    pub async fn remove_member(
        &self,
        project: &str,
        user_id: i64,
        protect_last_owner: bool,
    ) -> Result<Option<RemovedMember>, StorageError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(failed("begin remove member"))?;
        let removed: Option<String> = sqlx::query_scalar(
            "DELETE FROM project_members WHERE project = ? AND user_id = ? RETURNING role",
        )
        .bind(project)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(failed("remove member"))?;
        let Some(role) = removed else {
            return Ok(None);
        };
        let role = decode_role(&role)?;
        if protect_last_owner
            && role == Role::Owner
            && owner_count(&mut transaction, project).await? == 0
        {
            return Err(StorageError::LastOwner);
        }
        let username = username_of(&mut transaction, user_id).await?;
        transaction
            .commit()
            .await
            .map_err(failed("commit remove member"))?;
        Ok(Some(RemovedMember { username, role }))
    }

    /// Records a pending request. Returns whether a request was created; asking
    /// again while one is pending succeeds without change.
    pub async fn request_access(&self, project: &str, user_id: i64) -> Result<bool, StorageError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(failed("begin request access"))?;
        let inserted = sqlx::query(
            "INSERT INTO access_requests (project, user_id) VALUES (?, ?) \
             ON CONFLICT(project, user_id) DO NOTHING",
        )
        .bind(project)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(failed("request access"))?;
        if member_role(&mut transaction, project, user_id)
            .await?
            .is_some()
        {
            return Err(StorageError::AlreadyMember);
        }
        transaction
            .commit()
            .await
            .map_err(failed("commit request access"))?;
        Ok(inserted.rows_affected() == 1)
    }

    /// Deletes a pending request, for both a withdrawal and a denial. Returns
    /// the requester's username when a request existed.
    pub async fn delete_access_request(
        &self,
        project: &str,
        user_id: i64,
    ) -> Result<Option<String>, StorageError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(failed("begin delete request"))?;
        let deleted: Option<i64> = sqlx::query_scalar(
            "DELETE FROM access_requests WHERE project = ? AND user_id = ? RETURNING user_id",
        )
        .bind(project)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(failed("delete access request"))?;
        if deleted.is_none() {
            return Ok(None);
        }
        let username = username_of(&mut transaction, user_id).await?;
        transaction
            .commit()
            .await
            .map_err(failed("commit delete request"))?;
        Ok(Some(username))
    }
}

async fn ensure_registered(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &str,
) -> Result<(), StorageError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM project_registrations WHERE name = ?")
            .bind(project)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(failed("find project registration"))?;
    found
        .map(|_| ())
        .ok_or_else(|| StorageError::ProjectNotRegistered(project.to_string()))
}

async fn username_of(
    transaction: &mut Transaction<'_, Sqlite>,
    user_id: i64,
) -> Result<String, StorageError> {
    let username: Option<String> =
        sqlx::query_scalar("SELECT display_name FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(failed("find user"))?;
    username.ok_or(StorageError::IdentityNotFound(user_id))
}

async fn member_role(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &str,
    user_id: i64,
) -> Result<Option<Role>, StorageError> {
    let role: Option<String> =
        sqlx::query_scalar("SELECT role FROM project_members WHERE project = ? AND user_id = ?")
            .bind(project)
            .bind(user_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(failed("read member role"))?;
    role.map(|role| decode_role(&role)).transpose()
}

async fn owner_count(
    transaction: &mut Transaction<'_, Sqlite>,
    project: &str,
) -> Result<i64, StorageError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM project_members WHERE project = ? AND role = 'owner'")
        .bind(project)
        .fetch_one(&mut **transaction)
        .await
        .map_err(failed("count owners"))
}

fn decode_member(row: &SqliteRow) -> Result<MemberRecord, StorageError> {
    let role: String = row.try_get("role").map_err(failed("decode member"))?;
    Ok(MemberRecord {
        user_id: row.try_get("user_id").map_err(failed("decode member"))?,
        username: row.try_get("username").map_err(failed("decode member"))?,
        role: decode_role(&role)?,
        granted_by: row.try_get("granted_by").map_err(failed("decode member"))?,
        granted_at: row.try_get("granted_at").map_err(failed("decode member"))?,
    })
}

fn decode_role(value: &str) -> Result<Role, StorageError> {
    Role::parse(value).ok_or_else(|| StorageError::InvalidData {
        operation: "decode role",
        message: format!("unknown role {value:?}"),
    })
}

fn failed(operation: &'static str) -> impl FnOnce(sqlx::Error) -> StorageError {
    move |source| StorageError::Operation { operation, source }
}
