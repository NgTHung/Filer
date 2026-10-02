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

use std::path::Path;

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
        // Resolve aliases before walking parents; missing suffixes cannot hide ancestry.
        let mut ancestor = std::path::absolute(dst)
            .map_err(|error| CoreError::from_io_error(error, dst.to_path_buf()))?;
        loop {
            match std::fs::canonicalize(&ancestor) {
                Ok(existing) => {
                    for parent in existing.ancestors() {
                        let handle = Handle::from_path(parent).map_err(|error| {
                            CoreError::from_io_error(error, parent.to_path_buf())
                        })?;
                        if source == handle {
                            return Err(CoreError::invalid_input(
                                "A directory cannot be transferred into itself or a descendant",
                            ));
                        }
                    }
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    // A dangling symlink cannot be treated as a missing directory.
                    match std::fs::symlink_metadata(&ancestor) {
                        Ok(_) => return Err(CoreError::from_io_error(error, ancestor)),
                        Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(CoreError::from_io_error(error, ancestor)),
                    }
                    if !ancestor.pop() {
                        return Err(CoreError::from_io_error(error, dst.to_path_buf()));
                    }
                }
                Err(error) => return Err(CoreError::from_io_error(error, ancestor)),
            }
        }
    }
    Ok(())
}
