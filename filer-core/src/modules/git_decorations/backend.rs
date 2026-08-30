//! Git status providers used by the decoration actor.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

use crate::CoreError;
use crate::model::cancel::CancelSignal;
use crate::modules::git_decorations::{FileDecoration, FileDecorationState, GitDecorationTarget};

/// The repository roots needed to invalidate status after a filesystem change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepository {
    /// The worktree containing the visible paths.
    pub worktree: PathBuf,
    /// The metadata directory for this worktree.
    pub git_dir: PathBuf,
    /// The shared metadata directory containing refs and shared state.
    pub common_dir: PathBuf,
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
            .command_output(self.rev_parse_args(parent, "--show-toplevel"), cancel)
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

        let worktree = canonical_git_path(path_from_git_output(&output.stdout)?, parent)?;
        let git_dir = canonical_git_path(
            self.rev_parse_path(parent, "--absolute-git-dir", cancel)
                .await?,
            parent,
        )?;
        let common_dir = canonical_git_path(
            self.rev_parse_path(parent, "--git-common-dir", cancel)
                .await?,
            parent,
        )?;
        Ok(Some(GitRepository {
            worktree,
            git_dir,
            common_dir,
        }))
    }

    fn rev_parse_args(&self, parent: &Path, option: &str) -> Vec<OsString> {
        vec![
            OsString::from("--literal-pathspecs"),
            OsString::from("-C"),
            parent.as_os_str().to_os_string(),
            OsString::from("rev-parse"),
            OsString::from(option),
        ]
    }

    async fn rev_parse_path(
        &self,
        parent: &Path,
        option: &str,
        cancel: &CancelSignal,
    ) -> Result<PathBuf, CoreError> {
        let output = self
            .command_output(self.rev_parse_args(parent, option), cancel)
            .await?;
        if !output.status.success() {
            return Err(CoreError::io(
                parent.to_path_buf(),
                format!(
                    "Git repository path discovery failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        path_from_git_output(&output.stdout)
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
        let visible = visible.to_vec();
        let parent = absolute_path(parent)?;
        let path_cancel = cancel.clone();
        let normalized =
            tokio::task::spawn_blocking(move || normalize_paths(&parent, &visible, &path_cancel))
                .await
                .map_err(|error| {
                    CoreError::actor("git-decorations", format!("path worker failed: {error}"))
                })??;
        let (parent, normalized_visible) = normalized;
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

        let parse_cancel = cancel.clone();
        let parse_output = output.stdout;
        let parse_worktree = repository.worktree.clone();
        let parse_targets = normalized_visible;
        let decorations = tokio::task::spawn_blocking(move || {
            decorations_from_status(
                &parse_output,
                &parse_worktree,
                &parse_targets,
                &parse_cancel,
            )
        })
        .await
        .map_err(|error| {
            CoreError::actor("git-decorations", format!("status parser failed: {error}"))
        })??;
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

fn normalize_paths(
    parent: &Path,
    visible: &[GitDecorationTarget],
    cancel: &CancelSignal,
) -> Result<(PathBuf, Vec<GitDecorationTarget>), CoreError> {
    let parent = std::fs::canonicalize(parent).map_err(|error| {
        CoreError::io(
            parent.to_path_buf(),
            format!("cannot canonicalize Git parent: {error}"),
        )
    })?;
    let absolute_parent = absolute_path(&parent)?;
    let mut normalized_visible = Vec::with_capacity(visible.len());
    for target in visible {
        if cancel.is_cancelled() {
            return Err(CoreError::cancelled());
        }
        let absolute_target = absolute_path(&target.path)?;
        let path = if absolute_target == absolute_parent {
            parent.clone()
        } else if let Ok(relative) = absolute_target.strip_prefix(&absolute_parent) {
            append_relative(&parent, relative)
        } else {
            normalize_external_target(&absolute_target)?
        };
        normalized_visible.push(GitDecorationTarget {
            location: target.location.clone(),
            path,
        });
    }
    Ok((parent, normalized_visible))
}

fn append_relative(base: &Path, relative: &Path) -> PathBuf {
    let mut output = base.to_path_buf();
    for component in relative.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                output.pop();
            }
            std::path::Component::Normal(component) => output.push(component),
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {}
        }
    }
    output
}

fn normalize_external_target(path: &Path) -> Result<PathBuf, CoreError> {
    let Some(file_name) = path.file_name() else {
        return std::fs::canonicalize(path).map_err(|error| {
            CoreError::io(
                path.to_path_buf(),
                format!("cannot canonicalize Git target: {error}"),
            )
        });
    };
    let mut base = path.to_path_buf();
    let _ = base.pop();
    let mut suffix = vec![file_name.to_os_string()];
    while std::fs::symlink_metadata(&base).is_err() {
        let Some(component) = base.file_name() else {
            break;
        };
        suffix.push(component.to_os_string());
        if !base.pop() {
            break;
        }
    }
    let mut output = std::fs::canonicalize(&base).map_err(|error| {
        CoreError::io(
            path.to_path_buf(),
            format!("cannot canonicalize Git target parent: {error}"),
        )
    })?;
    for component in suffix.into_iter().rev() {
        output.push(component);
    }
    Ok(output)
}

fn path_from_git_output(output: &[u8]) -> Result<PathBuf, CoreError> {
    let path = output.strip_suffix(b"\n").ok_or_else(|| {
        CoreError::invalid_input("Git repository path discovery returned an invalid result")
    })?;
    if path.is_empty() {
        return Err(CoreError::invalid_input(
            "Git repository path discovery returned an empty path",
        ));
    }
    #[cfg(unix)]
    {
        Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(path.to_vec())
            .map(PathBuf::from)
            .map_err(|error| CoreError::invalid_input(format!("Git path is not UTF-8: {error}")))
    }
}

fn canonical_git_path(path: PathBuf, parent: &Path) -> Result<PathBuf, CoreError> {
    let path = if path.is_absolute() {
        path
    } else {
        parent.join(path)
    };
    std::fs::canonicalize(&path).map_err(|error| {
        CoreError::io(
            path,
            format!("cannot canonicalize Git repository path: {error}"),
        )
    })
}

fn parse_status_records(
    output: &[u8],
    worktree: &Path,
    cancel: &CancelSignal,
) -> Result<Vec<(PathBuf, FileDecorationState)>, CoreError> {
    let mut statuses = Vec::new();
    for record in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        if cancel.is_cancelled() {
            return Err(CoreError::cancelled());
        }
        let mut fields = record.splitn(2, |byte| *byte == b' ');
        let kind = fields.next().unwrap_or_default();
        let rest = fields.next().unwrap_or_default();
        let (state, path) = match kind {
            b"1" => {
                let mut fields = rest.splitn(8, |byte| *byte == b' ');
                let xy = fields.next().unwrap_or_default();
                let path = fields.nth(6).unwrap_or_default();
                (state_from_xy(xy, false), path)
            }
            b"u" => {
                let path = rest
                    .splitn(10, |byte| *byte == b' ')
                    .nth(9)
                    .unwrap_or_default();
                (FileDecorationState::Conflicted, path)
            }
            b"?" => (FileDecorationState::Untracked, rest),
            b"!" => (FileDecorationState::Ignored, rest),
            b"2" => {
                let mut fields = rest.splitn(9, |byte| *byte == b' ');
                let xy = fields.next().unwrap_or_default();
                let path = fields.nth(7).unwrap_or_default();
                (state_from_xy(xy, false), path)
            }
            _ => continue,
        };
        statuses.push((worktree.join(path_from_git_output_path(path)?), state));
    }
    Ok(statuses)
}

fn path_from_git_output_path(path: &[u8]) -> Result<OsString, CoreError> {
    #[cfg(unix)]
    {
        Ok(OsString::from_vec(path.to_vec()))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(path.to_vec())
            .map(OsString::from)
            .map_err(|error| {
                CoreError::invalid_input(format!("Git status path is not UTF-8: {error}"))
            })
    }
}

fn state_from_xy(xy: &[u8], unmerged: bool) -> FileDecorationState {
    if unmerged || xy.contains(&b'U') {
        FileDecorationState::Conflicted
    } else if xy.contains(&b'D') {
        FileDecorationState::Deleted
    } else if xy.contains(&b'A') {
        FileDecorationState::Added
    } else if xy.iter().any(|character| *character != b'.') {
        FileDecorationState::Modified
    } else {
        FileDecorationState::Clean
    }
}

fn decorations_from_status(
    output: &[u8],
    worktree: &Path,
    targets: &[GitDecorationTarget],
    cancel: &CancelSignal,
) -> Result<Vec<FileDecoration>, CoreError> {
    let statuses = parse_status_records(output, worktree, cancel)?;
    let mut target_indexes: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (index, target) in targets.iter().enumerate() {
        target_indexes
            .entry(target.path.clone())
            .or_default()
            .push(index);
    }
    let mut states = vec![FileDecorationState::Clean; targets.len()];
    for (status_path, status) in statuses {
        if cancel.is_cancelled() {
            return Err(CoreError::cancelled());
        }
        let mut ancestor = status_path;
        loop {
            if let Some(indices) = target_indexes.get(&ancestor) {
                for index in indices {
                    if decoration_priority(&status) > decoration_priority(&states[*index]) {
                        states[*index] = status;
                    }
                }
            }
            if !ancestor.pop() {
                break;
            }
        }
    }
    Ok(targets
        .iter()
        .zip(states)
        .map(|(target, state)| FileDecoration {
            location: target.location.clone(),
            state,
        })
        .collect())
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
