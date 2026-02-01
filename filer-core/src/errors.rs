use std::io::Error as IoError;
use std::path::PathBuf;

use flume::SendError;

#[derive(Debug)]
pub enum CoreError {
    /// IO error
    Io {
        path: PathBuf,
        message: String,
    },
    
    /// Path not found
    NotFound(PathBuf),
    
    /// Permission denied
    PermissionDenied(PathBuf),
    
    /// Invalid path
    InvalidPath(String),
    
    /// Channel closed
    ChannelClosed,
    
    /// Operation cancelled
    Cancelled,
    
    /// Actor error
    ActorError {
        actor: &'static str,
        message: String,
    },

    NetworkError,

    InvalidData,

    InvalidInput,
    
    Other(IoError),
    
    ChannelError(String)
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::Io { path, message } => write!(f,"I/O Error on path {:?}: {}.",path, message),
            CoreError::NotFound(path_buf) => write!(f,"Not found: {:?}.",path_buf),
            CoreError::PermissionDenied(path_buf) => write!(f,"Permission Denied on {:?}.",path_buf),
            CoreError::InvalidPath(p) => write!(f,"Invalid Path on {}.",p),
            CoreError::ChannelClosed => write!(f,"This Channel closed!"),
            CoreError::Cancelled => write!(f,"The operation was cancelled!"),
            CoreError::ActorError { actor, message } => write!(f,"Actor {} reported an Error: {}",actor,message),
            CoreError::NetworkError => write!(f,"Network error!"),
            CoreError::InvalidData => write!(f,"Invalid Data!"),
            CoreError::InvalidInput => write!(f,"Invalid Input!"),
            CoreError::Other(e) => write!(f,"Unknown error occured: {:?}",e),
            CoreError::ChannelError(e) => write!(f,"Channel error occured: {:?}",e),
            _ => write!(f,"Unknown error occured!")
        }
    }
}

impl std::error::Error for CoreError {}

impl CoreError {
    pub fn from_io_error(err: IoError, path: PathBuf) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound(path),
            std::io::ErrorKind::PermissionDenied => CoreError::PermissionDenied(path),
            std::io::ErrorKind::ConnectionRefused => CoreError::NetworkError,
            std::io::ErrorKind::ConnectionReset => CoreError::NetworkError,
            std::io::ErrorKind::HostUnreachable => CoreError::NetworkError,
            std::io::ErrorKind::NetworkUnreachable => CoreError::NetworkError,
            std::io::ErrorKind::ConnectionAborted => CoreError::NetworkError,
            std::io::ErrorKind::NotConnected => CoreError::NetworkError,
            std::io::ErrorKind::NetworkDown => CoreError::NetworkError,
            std::io::ErrorKind::BrokenPipe => CoreError::NetworkError,
            std::io::ErrorKind::AlreadyExists => CoreError::InvalidPath(path.to_str().unwrap_or_default().to_string()),
            std::io::ErrorKind::WouldBlock => CoreError::NetworkError,
            std::io::ErrorKind::NotADirectory => CoreError::InvalidPath(path.to_str().unwrap_or_default().to_string()),
            std::io::ErrorKind::IsADirectory => CoreError::InvalidPath(path.to_str().unwrap_or_default().to_string()),
            std::io::ErrorKind::DirectoryNotEmpty => CoreError::InvalidPath(path.to_str().unwrap_or_default().to_string()),
            std::io::ErrorKind::ReadOnlyFilesystem => CoreError::PermissionDenied(path),
            std::io::ErrorKind::StaleNetworkFileHandle => CoreError::NetworkError,
            _ => CoreError::Io { path, message: err.to_string() },
        }
    }
}