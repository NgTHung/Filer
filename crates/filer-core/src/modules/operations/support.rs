//! # Operation support
//!
//! This module centralizes provider context, event construction, and cache
//! invalidation so every operation handler applies the same lifecycle rules.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//!
//! let location = Location::local("/tmp/example");
//! let affected = LocationRef::from_location(&location);
//! assert_eq!(affected.id(), Some(location.id()));
//! ```

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::actors::cancel::CancellationToken;
use crate::services::dir_cache::SharedDirCache;
use crate::{CoreError, ErrorCode, ProviderCx};

pub(crate) type TrashFn = Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>;

pub(super) fn operation_cx(
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> ProviderCx<'_> {
    let cx = ProviderCx::with_cancel(cancel);
    match deadline {
        Some(deadline) => cx.with_deadline(deadline),
        None => cx,
    }
}

pub(super) fn is_cross_device(err: &CoreError) -> bool {
    err.code() == ErrorCode::IoFailed
        && (err.message.contains("cross-device")
            || err.message.contains("os error 18")
            || err.message.contains("os error 17"))
}

pub(super) fn invalidate_parent_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let (Some(parent), Some(c)) = (path.parent(), cache)
        && let Ok(mut guard) = c.lock()
    {
        guard.invalidate(crate::Location::local(parent.to_path_buf()).id());
    }
}

pub(super) fn invalidate_subtree_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let Some(c) = cache
        && let Ok(mut guard) = c.lock()
    {
        guard.invalidate_local_subtree(path);
    }
}
