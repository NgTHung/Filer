use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::location::{Location, LocationId};
use crate::model::node::NodeEntry;
use crate::vfs::provider::ListingOptions;

pub type SharedDirCache = Arc<Mutex<DirCache>>;

struct CacheEntry {
    location: Location,
    entries: Vec<NodeEntry>,
    size_bytes: usize,
    accessed: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CacheKey {
    location: LocationId,
    listing: ListingOptions,
}

/// LRU directory listing cache keyed by the directory's canonical location.
///
/// Rows retain their provider-owned locations, so a cache hit does not need a
/// path conversion or a registry lookup. Local subtree invalidation remains a
/// path operation because current write providers expose local paths, but it
/// only removes entries whose stored locations are local descendants.
pub struct DirCache {
    entries: HashMap<CacheKey, CacheEntry>,
    current_size_bytes: usize,
    max_size_bytes: usize,
}

impl DirCache {
    pub fn new(max_size_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            current_size_bytes: 0,
            max_size_bytes,
        }
    }

    /// Look up a directory listing by stable location identity.
    pub fn get(&mut self, location: LocationId, listing: ListingOptions) -> Option<Vec<NodeEntry>> {
        let entry = self.entries.get_mut(&CacheKey { location, listing })?;
        entry.accessed = Instant::now();
        Some(entry.entries.clone())
    }

    /// Insert or replace a listing, evicting least-recently-used entries.
    pub fn put(&mut self, location: Location, listing: ListingOptions, entries: Vec<NodeEntry>) {
        let key = CacheKey {
            location: location.id(),
            listing,
        };
        let size_bytes = estimate_size(&location, &entries);

        if let Some(old) = self.entries.remove(&key) {
            self.current_size_bytes -= old.size_bytes;
        }

        while !self.entries.is_empty()
            && self.current_size_bytes.saturating_add(size_bytes) > self.max_size_bytes
        {
            self.evict_lru();
        }

        self.current_size_bytes += size_bytes;
        self.entries.insert(
            key,
            CacheEntry {
                location,
                entries,
                size_bytes,
                accessed: Instant::now(),
            },
        );
    }

    /// Remove the listing for one location across all detail modes.
    pub fn invalidate(&mut self, location: LocationId) {
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|key| key.location == location)
            .copied()
            .collect();
        for key in keys {
            self.remove_key(&key);
        }
    }

    /// Remove local listings for `path` and all local descendant directories.
    pub fn invalidate_local_subtree(&mut self, path: &Path) {
        let keys: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                entry
                    .location
                    .as_local_path()
                    .filter(|cached| *cached == path || cached.starts_with(path))
                    .map(|_| *key)
            })
            .collect();
        for key in keys {
            self.remove_key(&key);
        }
    }

    /// Remove every cached listing.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size_bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current_size_bytes(&self) -> usize {
        self.current_size_bytes
    }

    fn remove_key(&mut self, key: &CacheKey) {
        if let Some(old) = self.entries.remove(key) {
            self.current_size_bytes -= old.size_bytes;
        }
    }

    fn evict_lru(&mut self) {
        let oldest_key = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.accessed)
            .map(|(key, _)| *key);
        if let Some(key) = oldest_key {
            self.remove_key(&key);
        }
    }
}

fn estimate_size(location: &Location, entries: &[NodeEntry]) -> usize {
    let location_size = std::mem::size_of::<Location>()
        + location.descriptor().root().as_os_str().len()
        + location.descriptor().scheme().len()
        + location.descriptor().display().len();
    entries
        .iter()
        .map(|entry| {
            std::mem::size_of::<NodeEntry>()
                + entry.name.capacity()
                + entry.display_path.as_ref().map_or(0, String::capacity)
                + entry
                    .location
                    .descriptor()
                    .map_or(0, |descriptor| descriptor.display().len())
        })
        .sum::<usize>()
        .saturating_add(location_size)
        .max(64)
}
