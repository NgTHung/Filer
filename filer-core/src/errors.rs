use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;
use std::path::PathBuf;

use flume::SendError;

use crate::model::location::LocationId;
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Io,
    NotFound,
    PermissionDenied,
    InvalidPath,
    InvalidLocation,
    ChannelClosed,
    Cancelled,
    Actor,
    Network,
    InvalidData,
    InvalidInput,
    Unsupported,
    Unknown,
}

impl ErrorKind {
    pub fn is_recoverable(self) -> bool {
        matches!(
            self,
            ErrorKind::NotFound
                | ErrorKind::PermissionDenied
                | ErrorKind::InvalidPath
                | ErrorKind::InvalidLocation
                | ErrorKind::Cancelled
                | ErrorKind::Network
                | ErrorKind::Unsupported
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    IoFailed,
    PathNotFound,
    PermissionDenied,
    ReadOnly,
    InvalidPath,
    LocationUnresolved,
    LocationSegmentedUnsupported,
    UnsupportedProvider,
    ChannelClosed,
    OperationCancelled,
    ActorFailed,
    NetworkFailed,
    DataInvalid,
    InputInvalid,
    SessionUnknown,
    NavigationUnavailable,
    UnsupportedOperation,
    Unknown,
}

impl ErrorCode {
    pub fn kind(self) -> ErrorKind {
        match self {
            ErrorCode::IoFailed => ErrorKind::Io,
            ErrorCode::PathNotFound => ErrorKind::NotFound,
            ErrorCode::PermissionDenied | ErrorCode::ReadOnly => ErrorKind::PermissionDenied,
            ErrorCode::InvalidPath => ErrorKind::InvalidPath,
            ErrorCode::LocationUnresolved | ErrorCode::LocationSegmentedUnsupported => {
                ErrorKind::InvalidLocation
            }
            ErrorCode::UnsupportedProvider | ErrorCode::UnsupportedOperation => {
                ErrorKind::Unsupported
            }
            ErrorCode::ChannelClosed => ErrorKind::ChannelClosed,
            ErrorCode::OperationCancelled => ErrorKind::Cancelled,
            ErrorCode::ActorFailed => ErrorKind::Actor,
            ErrorCode::NetworkFailed => ErrorKind::Network,
            ErrorCode::DataInvalid => ErrorKind::InvalidData,
            ErrorCode::InputInvalid
            | ErrorCode::SessionUnknown
            | ErrorCode::NavigationUnavailable => ErrorKind::InvalidInput,
            ErrorCode::Unknown => ErrorKind::Unknown,
        }
    }

    pub fn is_recoverable(self) -> bool {
        matches!(
            self,
            ErrorCode::PathNotFound
                | ErrorCode::PermissionDenied
                | ErrorCode::ReadOnly
                | ErrorCode::InvalidPath
                | ErrorCode::LocationUnresolved
                | ErrorCode::LocationSegmentedUnsupported
                | ErrorCode::UnsupportedProvider
                | ErrorCode::OperationCancelled
                | ErrorCode::NetworkFailed
                | ErrorCode::SessionUnknown
                | ErrorCode::NavigationUnavailable
                | ErrorCode::UnsupportedOperation
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorTarget {
    Path(PathBuf),
    Location(LocationId),
    Provider(String),
    Actor(&'static str),
    Request(RequestId),
    Operation(OperationId),
    Session(SessionId),
    Channel(&'static str),
}

#[derive(Debug)]
pub struct CoreError {
    pub kind: ErrorKind,
    pub code: ErrorCode,
    pub target: Option<ErrorTarget>,
    pub message: String,
    pub recoverable: bool,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl CoreError {
    pub fn new(code: ErrorCode, target: Option<ErrorTarget>, message: impl Into<String>) -> Self {
        Self {
            kind: code.kind(),
            code,
            target,
            message: message.into(),
            recoverable: code.is_recoverable(),
            source: None,
        }
    }

    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    pub fn io(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::IoFailed,
            Some(ErrorTarget::Path(path.into())),
            message,
        )
    }

    pub fn not_found(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self::new(
            ErrorCode::PathNotFound,
            Some(ErrorTarget::Path(path.clone())),
            format!("Not found: {}", path.display()),
        )
    }

    pub fn permission_denied(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self::new(
            ErrorCode::PermissionDenied,
            Some(ErrorTarget::Path(path.clone())),
            format!("Permission denied: {}", path.display()),
        )
    }

    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidPath, None, message)
    }

    pub fn invalid_location(
        code: ErrorCode,
        target: Option<ErrorTarget>,
        message: impl Into<String>,
    ) -> Self {
        debug_assert!(matches!(
            code,
            ErrorCode::LocationUnresolved
                | ErrorCode::LocationSegmentedUnsupported
                | ErrorCode::UnsupportedProvider
        ));
        Self::new(code, target, message)
    }

    pub fn location_unresolved(id: LocationId) -> Self {
        Self::invalid_location(
            ErrorCode::LocationUnresolved,
            Some(ErrorTarget::Location(id)),
            format!("Unresolved location id: {id}"),
        )
    }

    pub fn unsupported_provider(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::UnsupportedProvider,
            Some(ErrorTarget::Provider(provider.into())),
            message,
        )
    }

    pub fn channel_closed(channel: &'static str) -> Self {
        Self::new(
            ErrorCode::ChannelClosed,
            Some(ErrorTarget::Channel(channel)),
            format!("Channel closed: {channel}"),
        )
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorCode::OperationCancelled, None, "Operation cancelled")
    }

    pub fn actor(actor: &'static str, message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::ActorFailed,
            Some(ErrorTarget::Actor(actor)),
            message,
        )
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NetworkFailed, None, message)
    }

    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::DataInvalid, None, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InputInvalid, None, message)
    }

    pub fn unknown_session(session: SessionId) -> Self {
        Self::new(
            ErrorCode::SessionUnknown,
            Some(ErrorTarget::Session(session)),
            format!("Unknown session: {session}"),
        )
    }

    pub fn navigation_unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NavigationUnavailable, None, message)
    }

    pub fn unsupported_operation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::UnsupportedOperation, None, message)
    }

    pub fn other(error: IoError) -> Self {
        let message = error.to_string();
        Self::new(ErrorCode::Unknown, None, message).with_source(error)
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn target(&self) -> Option<&ErrorTarget> {
        self.target.as_ref()
    }

    pub fn recoverable(&self) -> bool {
        self.recoverable
    }

    pub fn emit_trace(&self) {
        let level = if self.recoverable {
            if matches!(
                self.code,
                ErrorCode::PermissionDenied
                    | ErrorCode::ReadOnly
                    | ErrorCode::UnsupportedProvider
                    | ErrorCode::UnsupportedOperation
            ) {
                TraceLevel::Warn
            } else {
                TraceLevel::Debug
            }
        } else {
            TraceLevel::Error
        };

        match level {
            TraceLevel::Debug => tracing::debug!(
                error.kind = ?self.kind,
                error.code = ?self.code,
                error.target = ?self.target,
                error.recoverable = self.recoverable,
                error.message = %self.message,
                "core error"
            ),
            TraceLevel::Warn => tracing::warn!(
                error.kind = ?self.kind,
                error.code = ?self.code,
                error.target = ?self.target,
                error.recoverable = self.recoverable,
                error.message = %self.message,
                "core error"
            ),
            TraceLevel::Error => tracing::error!(
                error.kind = ?self.kind,
                error.code = ?self.code,
                error.target = ?self.target,
                error.recoverable = self.recoverable,
                error.message = %self.message,
                "core error"
            ),
        }
    }

    pub fn from_io_error(err: IoError, path: PathBuf) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::not_found(path).with_source(err),
            std::io::ErrorKind::PermissionDenied => Self::permission_denied(path).with_source(err),
            std::io::ErrorKind::ReadOnlyFilesystem => Self::new(
                ErrorCode::ReadOnly,
                Some(ErrorTarget::Path(path.clone())),
                format!("Read-only filesystem: {}", path.display()),
            )
            .with_source(err),
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
                Self::network(format!("{} ({})", err, path.display())).with_source(err)
            }
            std::io::ErrorKind::AlreadyExists
            | std::io::ErrorKind::NotADirectory
            | std::io::ErrorKind::IsADirectory
            | std::io::ErrorKind::DirectoryNotEmpty => {
                Self::invalid_path(path.display().to_string()).with_source(err)
            }
            _ => Self::io(path, err.to_string()).with_source(err),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TraceLevel {
    Debug,
    Warn,
    Error,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for CoreError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &(dyn StdError + 'static))
    }
}

impl<T: fmt::Debug> From<SendError<T>> for CoreError {
    fn from(_: SendError<T>) -> Self {
        CoreError::channel_closed("send")
    }
}
