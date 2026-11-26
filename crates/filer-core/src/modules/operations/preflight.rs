//! # Transfer preflight
//!
//! Validate every target before a batch writes anything, so a later invalid
//! source cannot leave earlier destinations changed. Copy and Move use the same
//! checks so provider limitations produce consistent errors.
//!
//! ```
//! use filer_core::modules::operations::preflight::check_transfer;
//! use filer_core::{FsProvider, ProviderCx};
//! use std::path::{Path, PathBuf};
//!
//! async fn validate(provider: &dyn FsProvider, sources: &[PathBuf], parent: &Path)
//!     -> Result<(), filer_core::CoreError>
//! {
//!     check_transfer(provider, sources, parent, &ProviderCx::none()).await
//! }
//! ```

use std::path::{Path, PathBuf};

use crate::{CoreError, FsProvider, ProviderCx};

pub async fn check_transfer(
    provider: &dyn FsProvider,
    sources: &[PathBuf],
    destination: &Path,
    cx: &ProviderCx<'_>,
) -> Result<(), CoreError> {
    validated_targets(provider, sources, destination, cx)
        .await
        .map(|_| ())
}

pub(super) async fn validated_targets(
    provider: &dyn FsProvider,
    sources: &[PathBuf],
    destination: &Path,
    cx: &ProviderCx<'_>,
) -> Result<Vec<PathBuf>, CoreError> {
    let mut targets = Vec::with_capacity(sources.len());
    for source in sources {
        let name = source
            .file_name()
            .ok_or_else(|| CoreError::invalid_input("Transfer source must have a file name"))?;
        let target = destination.join(name);
        cx.race(
            provider.scheme(),
            provider.preflight_transfer(source, &target, cx),
        )
        .await?;
        targets.push(target);
    }
    Ok(targets)
}
