use std::path::PathBuf;
use std::time::SystemTime;

/// Basic filesystem metadata (always available)
#[derive(Debug, Clone)]
pub struct BasicMetadata {
    pub path: PathBuf,
    pub size: u64,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub permissions: Permissions,
    pub is_symlink: bool,
    pub symlink_target: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Permissions {
    pub readonly: bool,
    pub hidden: bool,
    pub mode: Option<u32>,        // Unix mode
    pub owner: Option<String>,
    pub group: Option<String>,
}

impl BasicMetadata {
    /// Load basic metadata from path
    pub async fn from_path(path: PathBuf) -> Result<Self, std::io::Error> {
        todo!()
    }

    /// Human-readable size string
    pub fn size_formatted(&self) -> String {
        todo!()
    }
}