//! Git status providers used by the decoration actor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use crate::CoreError;
use crate::model::cancel::CancelSignal;
use crate::modules::git_decorations::{FileDecoration, FileDecorationState, GitDecorationTarget};

/// The repository roots needed to invalidate status after a filesystem change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepository {
    pub worktree: PathBuf,
    pub git_dir: PathBuf,
}

/// Result returned by a Git status backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusResult {
    pub repository: Option<GitRepository>,
    pub decorations: Vec<FileDecoration>,
}

/// Injectable asynchronous source of semantic Git decorations.
#[async_trait]
pub trait GitStatusBackend: Send + Sync + 'static {
    async fn status(
        &self,
        parent: &Path,
        visible: &[GitDecorationTarget],
        cancel: &CancelSignal,
    ) -> Result<GitStatusResult, CoreError>;
}

/// Git CLI implementation of [`GitStatusBackend`].
#[derive(Debug, Clone)]
pub struct GitCliBackend {
    program: PathBuf,
}

impl GitCliBackend {
    pub fn new() -> Self {
        Self {
            program: PathBuf::from("git"),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    async fn command_output(
        &self,
        args: impl IntoIterator<Item = std::ffi::OsString>,
        cancel: &CancelSignal,
    ) -> Result<std::process::Output, CoreError> {
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = command.spawn().map_err(|error| {
            CoreError::unsupported_operation(format!(
                "unable to execute Git at {}: {error}",
                self.program.display()
            ))
        })?;
        tokio::select! {
            output = child.wait_with_output() => output.map_err(|error| {
                CoreError::io(PathBuf::new(), format!("Git process failed: {error}"))
            }),
            _ = cancel.cancelled() => Err(CoreError::cancelled()),
        }
    }

    async fn repository(
        &self,
        parent: &Path,
        cancel: &CancelSignal,
    ) -> Result<Option<GitRepository>, CoreError> {
        let output = self
            .command_output(
                [
                    std::ffi::OsString::from("--literal-pathspecs"),
                    std::ffi::OsString::from("-C"),
                    parent.as_os_str().to_os_string(),
                    std::ffi::OsString::from("rev-parse"),
                    std::ffi::OsString::from("--show-toplevel"),
                    std::ffi::OsString::from("--absolute-git-dir"),
                ],
                cancel,
            )
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not a git repository") {
                return Ok(None);
            }
            return Err(CoreError::io(
                parent.to_path_buf(),
                format!("Git repository discovery failed: {}", stderr.trim()),
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<_> = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        if lines.len() != 2 {
            return Err(CoreError::invalid_input(
                "Git repository discovery returned an invalid result",
            ));
        }
        Ok(Some(GitRepository {
            worktree: PathBuf::from(lines[0]),
            git_dir: PathBuf::from(lines[1]),
        }))
    }
}

impl Default for GitCliBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitStatusBackend for GitCliBackend {
    async fn status(
        &self,
        parent: &Path,
        visible: &[GitDecorationTarget],
        cancel: &CancelSignal,
    ) -> Result<GitStatusResult, CoreError> {
        let parent = absolute_path(parent)?;
        let normalized_visible: Vec<_> = visible
            .iter()
            .map(|target| {
                Ok(GitDecorationTarget {
                    location: target.location.clone(),
                    path: absolute_path(&target.path)?,
                })
            })
            .collect::<Result<_, CoreError>>()?;
        let Some(repository) = self.repository(&parent, cancel).await? else {
            return Ok(GitStatusResult {
                repository: None,
                decorations: Vec::new(),
            });
        };

        if normalized_visible.is_empty() {
            return Ok(GitStatusResult {
                repository: Some(repository),
                decorations: Vec::new(),
            });
        }

        let mut pathspecs = Vec::with_capacity(normalized_visible.len());
        for target in &normalized_visible {
            let relative = target
                .path
                .strip_prefix(&repository.worktree)
                .map_err(|_| {
                    CoreError::invalid_input(format!(
                        "visible decoration path is outside repository: {}",
                        target.path.display()
                    ))
                })?;
            pathspecs.push(if relative.as_os_str().is_empty() {
                std::ffi::OsString::from(".")
            } else {
                relative.as_os_str().to_os_string()
            });
        }

        let mut args = vec![
            std::ffi::OsString::from("--literal-pathspecs"),
            std::ffi::OsString::from("-C"),
            repository.worktree.as_os_str().to_os_string(),
            std::ffi::OsString::from("status"),
            std::ffi::OsString::from("--porcelain=v2"),
            std::ffi::OsString::from("--ignored=matching"),
            std::ffi::OsString::from("--untracked-files=all"),
            std::ffi::OsString::from("--no-renames"),
            std::ffi::OsString::from("-z"),
            std::ffi::OsString::from("--"),
        ];
        args.extend(pathspecs);
        let output = self.command_output(args, cancel).await?;
        if !output.status.success() {
            return Err(CoreError::io(
                repository.worktree.clone(),
                format!(
                    "Git status failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }

        let statuses = parse_status_records(&output.stdout, &repository.worktree)?;
        let mut decorations = Vec::with_capacity(normalized_visible.len());
        for target in &normalized_visible {
            let state = state_for_target(&target.path, &statuses);
            decorations.push(FileDecoration {
                location: target.location.clone(),
                state,
            });
        }
        Ok(GitStatusResult {
            repository: Some(repository),
            decorations,
        })
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, CoreError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| CoreError::io(path.to_path_buf(), format!("cannot resolve path: {error}")))
}

fn parse_status_records(
    output: &[u8],
    worktree: &Path,
) -> Result<HashMap<PathBuf, FileDecorationState>, CoreError> {
    let mut statuses = HashMap::new();
    for record in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let text = std::str::from_utf8(record).map_err(|error| {
            CoreError::invalid_input(format!("Git status is not UTF-8: {error}"))
        })?;
        let mut fields = text.splitn(2, ' ');
        let kind = fields.next().unwrap_or_default();
        let rest = fields.next().unwrap_or_default();
        let (state, path) = match kind {
            "1" => {
                let mut fields = rest.splitn(8, ' ');
                let xy = fields.next().unwrap_or_default();
                let path = fields.nth(6).unwrap_or_default();
                (state_from_xy(xy, false), path)
            }
            "u" => {
                let path = rest.splitn(10, ' ').nth(9).unwrap_or_default();
                (FileDecorationState::Conflicted, path)
            }
            "?" => (FileDecorationState::Untracked, rest),
            "!" => (FileDecorationState::Ignored, rest),
            "2" => {
                let mut fields = rest.splitn(9, ' ');
                let xy = fields.next().unwrap_or_default();
                let path = fields.nth(7).unwrap_or_default();
                (state_from_xy(xy, false), path)
            }
            _ => continue,
        };
        statuses.insert(worktree.join(path), state);
    }
    Ok(statuses)
}

fn state_from_xy(xy: &str, unmerged: bool) -> FileDecorationState {
    if unmerged || xy.contains('U') {
        FileDecorationState::Conflicted
    } else if xy.contains('D') {
        FileDecorationState::Deleted
    } else if xy.contains('A') {
        FileDecorationState::Added
    } else if xy.chars().any(|character| character != '.') {
        FileDecorationState::Modified
    } else {
        FileDecorationState::Clean
    }
}

fn state_for_target(
    target: &Path,
    statuses: &HashMap<PathBuf, FileDecorationState>,
) -> FileDecorationState {
    statuses
        .iter()
        .filter(|(path, _)| path.as_path() == target || path.starts_with(target))
        .map(|(_, state)| *state)
        .max_by_key(decoration_priority)
        .unwrap_or(FileDecorationState::Clean)
}

fn decoration_priority(state: &FileDecorationState) -> u8 {
    match state {
        FileDecorationState::Clean => 0,
        FileDecorationState::Ignored => 1,
        FileDecorationState::Untracked => 2,
        FileDecorationState::Modified => 3,
        FileDecorationState::Added => 4,
        FileDecorationState::Deleted => 5,
        FileDecorationState::Conflicted => 6,
    }
}
