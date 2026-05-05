use std::io::Error as IoError;
use std::path::PathBuf;

use flume::SendError;

#[derive(Debug)]
pub enum CoreError {
    /// IO error with path context
    Io { path: PathBuf, message: String },

    /// Path not found
    NotFound(PathBuf),

    /// Permission denied
    PermissionDenied(PathBuf),

    /// Invalid path (already exists, not a directory, etc.)
    InvalidPath(String),

    /// Channel closed — the receiving end has been dropped
    ChannelClosed(String),

    /// Operation cancelled
    Cancelled,

    /// Actor error (named actor reported a failure)
    ActorError {
        actor: &'static str,
        message: String,
    },

    /// Network-related error (connection refused, timeout, etc.)
    NetworkError(String),

    /// Invalid or corrupt data encountered
    InvalidData(String),

    /// Invalid input from client/caller
    InvalidInput(String),

    /// Wraps an underlying `std::io::Error` that doesn't fit other variants
    Other(IoError),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::Io { path, message } => {
                write!(f, "I/O error on {}: {}", path.display(), message)
            }
            CoreError::NotFound(path) => {
                write!(f, "Not found: {}", path.display())
            }
            CoreError::PermissionDenied(path) => {
                write!(f, "Permission denied: {}", path.display())
            }
            CoreError::InvalidPath(detail) => {
                write!(f, "Invalid path: {}", detail)
            }
            CoreError::ChannelClosed(detail) => {
                write!(f, "Channel closed: {}", detail)
            }
            CoreError::Cancelled => {
                write!(f, "Operation cancelled")
            }
            CoreError::ActorError { actor, message } => {
                write!(f, "Actor '{}' error: {}", actor, message)
            }
            CoreError::NetworkError(detail) => {
                write!(f, "Network error: {}", detail)
            }
            CoreError::InvalidData(detail) => {
                write!(f, "Invalid data: {}", detail)
            }
            CoreError::InvalidInput(detail) => {
                write!(f, "Invalid input: {}", detail)
            }
            CoreError::Other(e) => {
                write!(f, "Unexpected error: {}", e)
            }
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CoreError::Other(e) => Some(e),
            _ => None,
        }
    }
}

// ── From impls ───────────────────────────────────────────────────────

impl<T: std::fmt::Debug> From<SendError<T>> for CoreError {
    fn from(err: SendError<T>) -> Self {
        CoreError::ChannelClosed(err.to_string())
    }
}

// ── Contextual conversion ────────────────────────────────────────────

impl CoreError {
    /// Convert an `io::Error` into a `CoreError` with path context.
    ///
    /// Maps well-known `ErrorKind` variants to specific `CoreError`
    /// variants (NotFound, PermissionDenied, NetworkError, etc.).
    /// Unknown kinds fall through to `CoreError::Io { path, message }`.
    pub fn from_io_error(err: IoError, path: PathBuf) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound(path),
            std::io::ErrorKind::PermissionDenied => CoreError::PermissionDenied(path),
            std::io::ErrorKind::ReadOnlyFilesystem => CoreError::PermissionDenied(path),
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::NetworkDown
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::StaleNetworkFileHandle => {
                CoreError::NetworkError(format!("{} ({})", err, path.display()))
            }
            std::io::ErrorKind::AlreadyExists
            | std::io::ErrorKind::NotADirectory
            | std::io::ErrorKind::IsADirectory
            | std::io::ErrorKind::DirectoryNotEmpty => {
                CoreError::InvalidPath(path.display().to_string())
            }
            _ => CoreError::Io {
                path,
                message: err.to_string(),
            },
        }
    }
}
