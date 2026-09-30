//! # Project Registration Storage
//!
//! Persists the stable public name and canonical root for each registered
//! project. Roots use the platform's native representation so persistence does
//! not reject otherwise valid non-UTF-8 paths.

use std::{ffi::OsString, path::PathBuf};

use sqlx::Row;

use super::{Storage, StorageError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRegistration {
    pub name: String,
    pub root: PathBuf,
}

impl ProjectRegistration {
    pub fn new(name: impl Into<String>, root: PathBuf) -> Self {
        Self {
            name: name.into(),
            root,
        }
    }
}

impl Storage {
    pub async fn project_registrations(&self) -> Result<Vec<ProjectRegistration>, StorageError> {
        let rows = sqlx::query("SELECT name, root FROM project_registrations ORDER BY position")
            .fetch_all(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "load project registrations",
                source,
            })?;
        rows.into_iter()
            .map(|row| {
                let name: String =
                    row.try_get("name")
                        .map_err(|source| StorageError::Operation {
                            operation: "decode project registration name",
                            source,
                        })?;
                let root: Vec<u8> =
                    row.try_get("root")
                        .map_err(|source| StorageError::Operation {
                            operation: "decode project registration root",
                            source,
                        })?;
                Ok(ProjectRegistration::new(name, decode_path(root)?))
            })
            .collect()
    }

    pub async fn insert_project_registration(
        &self,
        registration: &ProjectRegistration,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO project_registrations (name, root) VALUES (?, ?)")
            .bind(&registration.name)
            .bind(encode_path(&registration.root))
            .execute(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "insert project registration",
                source,
            })?;
        Ok(())
    }

    pub async fn delete_project_registration(&self, name: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM project_registrations WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|source| StorageError::Operation {
                operation: "delete project registration",
                source,
            })?;
        Ok(result.rows_affected() == 1)
    }
}

#[cfg(unix)]
fn encode_path(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn decode_path(bytes: Vec<u8>) -> Result<PathBuf, StorageError> {
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(windows)]
fn encode_path(path: &std::path::Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn decode_path(bytes: Vec<u8>) -> Result<PathBuf, StorageError> {
    use std::os::windows::ffi::OsStringExt;

    if !bytes.len().is_multiple_of(2) {
        return Err(StorageError::InvalidData {
            operation: "decode project registration root",
            message: "Windows path data contains an incomplete UTF-16 code unit".to_string(),
        });
    }
    let wide = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    Ok(PathBuf::from(OsString::from_wide(&wide)))
}
