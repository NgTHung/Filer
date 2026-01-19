use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum FsChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}