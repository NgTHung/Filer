//! # Scan execution inputs
//!
//! These inputs keep a scan's target, event correlation, and borrowed runtime
//! resources together as work moves from dispatch to result emission.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//! let parent = LocationRef::from_location(&Location::local("/tmp"));
//! assert!(parent.descriptor().is_some());
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::actors::cancel::CancellationToken;
use crate::api::event_sink::EventSink;
use crate::model::location::{LocationId, LocationRef};
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::vfs::provider::FsProvider;

use super::paging::PagingSessions;

pub(super) struct ScanTarget {
    pub path: PathBuf,
    pub parent_location: LocationRef,
    pub parent_location_id: Option<LocationId>,
}

#[derive(Clone, Copy)]
pub(super) struct ScanEvents<'a> {
    pub events: &'a EventSink,
    pub latest_scans: &'a scc::HashMap<SessionId, RequestId, RandomState>,
    pub session: SessionId,
    pub request: RequestId,
}

pub(super) struct ScanResources<'a> {
    pub provider: &'a Arc<dyn FsProvider>,
    pub cancel: &'a CancellationToken,
    pub cache: Option<&'a SharedDirCache>,
    pub paging: &'a PagingSessions,
}
