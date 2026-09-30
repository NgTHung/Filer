//! # Node fixtures
//!
//! Both test harnesses share location identity and capability defaults here so
//! equivalent rows cannot drift between actor and integration tests.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//!
//! let location = LocationRef::from_location(&Location::local("/test/readme.txt"));
//! assert!(location.descriptor().is_some());
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::core::model::node::{NodeEntry, NodeKind, NodeMeta};
use super::core::{Location, LocationRef};

pub(crate) fn make_entry(
    path: impl Into<PathBuf>,
    name: impl Into<String>,
    kind: NodeKind,
    size: u64,
    modified: Option<std::time::SystemTime>,
    meta: NodeMeta,
) -> NodeEntry {
    let location = Location::local(path);
    NodeEntry {
        location: LocationRef::from_location(&location),
        display_path: None,
        capabilities: super::core::NodeEntryCapabilities {
            read: true,
            navigate: matches!(kind, NodeKind::Directory { .. }),
        },
        name: name.into(),
        kind,
        size,
        modified,
        created: None,
        accessed: None,
        meta,
    }
}

pub(crate) fn file(name: &str, parent: &str, size: u64) -> NodeEntry {
    make_entry(
        Path::new(parent).join(name),
        name,
        NodeKind::File {
            extension: Path::new(name)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_owned),
        },
        size,
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        NodeMeta::default(),
    )
}

pub(crate) fn directory(name: &str, parent: &str) -> NodeEntry {
    make_entry(
        Path::new(parent).join(name),
        name,
        NodeKind::Directory {
            children_count: None,
        },
        0,
        Some(SystemTime::UNIX_EPOCH),
        NodeMeta::default(),
    )
}
