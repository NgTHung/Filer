use std::path::PathBuf;

use rapidhash::fast::RandomState;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::provider::PreviewData;

/// Cached preview entry
struct CacheEntry {
    data: PreviewData,
    created: Instant,
    size_bytes: usize,
}

/// LRU cache for previews
pub struct PreviewCache {
    entries: Arc<scc::HashMap<PathBuf, CacheEntry, RandomState>>,
    max_size_bytes: usize,
    current_size_bytes: usize,
    ttl: Duration,
}

impl PreviewCache {
    pub fn new(max_size_bytes: usize, ttl: Duration) -> Self {
        Self {
            entries: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            max_size_bytes,
            current_size_bytes: 0,
            ttl,
        }
    }

    /// Get cached preview if valid
    pub fn get(&self, path: &PathBuf) -> Option<&PreviewData> {
        todo!()
    }

    /// Store preview in cache
    pub fn put(&mut self, path: PathBuf, data: PreviewData) {
        todo!()
    }

    /// Invalidate cache entry
    pub fn invalidate(&mut self, path: &PathBuf) {
        todo!()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        todo!()
    }

    /// Evict oldest entries to make room
    fn evict(&mut self, needed_bytes: usize) {
        todo!()
    }

    /// Estimate size of preview data
    fn estimate_size(data: &PreviewData) -> usize {
        todo!()
    }
}