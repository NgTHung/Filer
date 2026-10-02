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
        Ok(target) if source == target => Err(CoreError::invalid_input(
            "Transfer source and destination identify the same filesystem object",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CoreError::from_io_error(error, dst.to_path_buf())),
    }
}
