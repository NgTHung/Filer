//! # Local transfer preflight
//!
//! Filesystem handles identify aliases that path comparison misses. Checks run
//! on a blocking worker so identity lookup does not stall command dispatch.
//!
//! ```
//! use filer_core::{FsProvider, LocalFs, ProviderCx, ErrorCode};
//! # async fn example() -> Result<(), filer_core::CoreError> {
//! let directory = tempfile::tempdir().unwrap();
//! let file = directory.path().join("source.txt");
//! std::fs::write(&file, b"preserve me").unwrap();
//! let error = LocalFs::new().preflight_transfer(&file, &file, &ProviderCx::none()).await.unwrap_err();
//! assert_eq!(error.code(), ErrorCode::InputInvalid);
//! # Ok(())
//! # }
//! ```

use std::path::{Component, Path, PathBuf};

use same_file::Handle;

use crate::CoreError;

pub(super) fn check(src: &Path, dst: &Path) -> Result<(), CoreError> {
    let source = Handle::from_path(src)
        .map_err(|error| CoreError::from_io_error(error, src.to_path_buf()))?;
    match Handle::from_path(dst) {
        Ok(target) if source == target => {
            return Err(CoreError::invalid_input(
                "Transfer source and destination identify the same filesystem object",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(CoreError::from_io_error(error, dst.to_path_buf())),
    }

    let metadata = std::fs::metadata(src)
        .map_err(|error| CoreError::from_io_error(error, src.to_path_buf()))?;
    if metadata.is_dir() {
        let target = resolve_target(dst)?;
        for parent in target.ancestors() {
            match Handle::from_path(parent) {
                Ok(handle) if source == handle => {
                    return Err(CoreError::invalid_input(
                        "A directory cannot be transferred into itself or a descendant",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(CoreError::from_io_error(error, parent.to_path_buf())),
            }
        }
    }
    Ok(())
}

fn resolve_target(path: &Path) -> Result<PathBuf, CoreError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| CoreError::from_io_error(error, path.to_path_buf()))?;
    let mut resolved = PathBuf::new();
    // Resolve symlinks before applying parent components, including after missing suffixes.
    for component in absolute.components() {
        if component == Component::ParentDir {
            resolved.pop();
            continue;
        }
        resolved.push(component.as_os_str());
        if !matches!(component, Component::Normal(_)) {
            continue;
        }
        match std::fs::canonicalize(&resolved) {
            Ok(existing) => resolved = existing,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::symlink_metadata(&resolved) {
                    Ok(_) => return Err(CoreError::from_io_error(error, resolved)),
                    Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(CoreError::from_io_error(error, resolved)),
                }
            }
            Err(error) => return Err(CoreError::from_io_error(error, resolved)),
        }
    }
    Ok(resolved)
}
