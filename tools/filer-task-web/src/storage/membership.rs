//! # Membership Storage
//!
//! Keeps project roles behind the storage API so callers do not depend on SQL.
//!
//! ```
//! use filer_task_web::storage::Role;
//! assert!(Role::Owner.satisfies(Role::Editor));
//! assert_eq!(Role::parse("owner"), Some(Role::Owner));
//! ```

use super::{Storage, StorageError};
use serde::{Deserialize, Serialize};
use std::fmt;

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
