//! # Activity Storage
//!
//! Records one row per successful web-driven write. The username is
//! denormalized at write time rather than joined from `users`, since a later
//! rename should not rewrite history: the audit trail reflects who made a
//! change under the name they used then.

use sqlx::Row;

use super::{Storage, StorageError};

pub struct NewActivity<'a> {
    pub user_id: i64,
    pub username: &'a str,
    pub project: &'a str,
    pub task_id: Option<&'a str>,
    pub action: &'a str,
    pub detail: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityRecord {
    pub id: i64,
    pub created_at: i64,
    pub username: String,
    pub project: String,
    pub task_id: Option<String>,
    pub action: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ActivityFilter {
    pub project: Option<String>,
    pub task_id: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl Storage {
    pub async fn record_activity(&self, entry: NewActivity<'_>) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO activity (user_id, username, project, task_id, action, detail) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(entry.user_id)
        .bind(entry.username)
        .bind(entry.project)
        .bind(entry.task_id)
        .bind(entry.action)
        .bind(entry.detail)
        .execute(&self.pool)
        .await
        .map_err(|source| StorageError::Operation {
            operation: "record activity",
            source,
        })?;
        Ok(())
    }

    /// Records a row for a write whose task files already changed on disk.
    ///
    /// A recording failure is logged instead of returned. The mutation is
    /// already committed, so answering with an error would tell the client a
    /// change failed that actually succeeded, inviting a retry that
    /// double-applies it. Losing one audit row is the smaller loss.
    pub async fn record_committed_activity(&self, entry: NewActivity<'_>) {
        let (action, project) = (entry.action, entry.project);
        if let Err(error) = self.record_activity(entry).await {
            tracing::error!(action, project, %error, "activity recording failed for a committed write");
        }
    }

    pub async fn list_activity(
        &self,
        filter: ActivityFilter,
    ) -> Result<Vec<ActivityRecord>, StorageError> {
        let mut sql = String::from(
            "SELECT id, created_at, username, project, task_id, action, detail FROM activity",
        );
        let mut conditions = Vec::new();
        if filter.project.is_some() {
            conditions.push("project = ?");
        }
        if filter.task_id.is_some() {
            conditions.push("task_id = ?");
        }
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");

        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        if let Some(project) = &filter.project {
            query = query.bind(project);
        }
        if let Some(task_id) = &filter.task_id {
            query = query.bind(task_id);
        }
        query = query.bind(filter.limit).bind(filter.offset);

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "list activity",
                source,
            })?;
        rows.into_iter()
            .map(|row| decode_activity(&row))
            .collect()
    }
}

fn decode_activity(row: &sqlx::sqlite::SqliteRow) -> Result<ActivityRecord, StorageError> {
    let operation = "decode activity row";
    Ok(ActivityRecord {
        id: row
            .try_get("id")
            .map_err(|source| StorageError::Operation { operation, source })?,
        created_at: row
            .try_get("created_at")
            .map_err(|source| StorageError::Operation { operation, source })?,
        username: row
            .try_get("username")
            .map_err(|source| StorageError::Operation { operation, source })?,
        project: row
            .try_get("project")
            .map_err(|source| StorageError::Operation { operation, source })?,
        task_id: row
            .try_get("task_id")
            .map_err(|source| StorageError::Operation { operation, source })?,
        action: row
            .try_get("action")
            .map_err(|source| StorageError::Operation { operation, source })?,
        detail: row
            .try_get("detail")
            .map_err(|source| StorageError::Operation { operation, source })?,
    })
}
