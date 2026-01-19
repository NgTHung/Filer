use std::path::PathBuf;

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
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl std::error::Error for CoreError {}