use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rapidhash::fast::RandomState;

use super::provider::PreviewData;

struct CacheEntry {
    data: PreviewData,
    created: Instant,
    size_bytes: usize,
}

/// Size- and TTL-bounded preview cache.
///
/// Eviction strategy: on `put`, expired entries are removed first. If the
/// cache is still over capacity after TTL eviction, the entire cache is
/// cleared (simple but sufficient for a single-user file manager).
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

    /// Return a clone of the cached preview if the entry exists and has not expired.
    pub fn get(&self, path: &PathBuf) -> Option<PreviewData> {
        self.entries
            .read_sync(path, |_, entry| {
                if entry.created.elapsed() < self.ttl {
                    Some(entry.data.clone())
                } else {
                    None
                }
            })
            .flatten()
    }

    /// Insert or replace a preview, evicting if necessary.
    pub fn put(&mut self, path: PathBuf, data: PreviewData) {
        let size = Self::estimate_size(&data);

        // Remove any existing entry for this path first.
        if let Some((_, old)) = self.entries.remove_sync(&path) {
            self.current_size_bytes = self.current_size_bytes.saturating_sub(old.size_bytes);
        }

        if self.current_size_bytes + size > self.max_size_bytes {
            self.evict(size);
        }

        let entry = CacheEntry {
            data,
            created: Instant::now(),
            size_bytes: size,
        };
        if self.entries.insert_sync(path, entry).is_ok() {
            self.current_size_bytes += size;
        }
    }

    /// Remove a single entry from the cache.
    pub fn invalidate(&mut self, path: &PathBuf) {
        if let Some((_, entry)) = self.entries.remove_sync(path) {
            self.current_size_bytes = self.current_size_bytes.saturating_sub(entry.size_bytes);
        }
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear_sync();
        self.current_size_bytes = 0;
    }

    /// Remove expired entries. If still over capacity, clear everything.
    fn evict(&mut self, needed_bytes: usize) {
        let ttl = self.ttl;
        let mut freed = 0usize;
        self.entries.retain_sync(|_, entry| {
            if entry.created.elapsed() >= ttl {
                freed += entry.size_bytes;
                false
            } else {
                true
            }
        });
        self.current_size_bytes = self.current_size_bytes.saturating_sub(freed);

        // Nuclear option: if TTL eviction wasn't enough, clear everything.
        if self.current_size_bytes + needed_bytes > self.max_size_bytes {
            self.entries.clear_sync();
            self.current_size_bytes = 0;
        }
    }

    /// Estimate the in-memory footprint of a `PreviewData` value.
    fn estimate_size(data: &PreviewData) -> usize {
        match data {
            PreviewData::Text { content, .. } => content.len(),
            PreviewData::HighlightedText { content, .. } => content.len(),
            PreviewData::Image { data, .. } => data.len(),
            PreviewData::Audio {
                waveform,
                album_art,
                ..
            } => {
                waveform.as_ref().map(|w| w.len() * 4).unwrap_or(0)
                    + album_art.as_ref().map(|a| a.len()).unwrap_or(0)
            }
            PreviewData::Video { thumbnails, .. } => thumbnails.iter().map(|t| t.data.len()).sum(),
            PreviewData::Document { pages, .. } => pages.iter().map(|p| p.image.len()).sum(),
            PreviewData::Archive { entries, .. } => entries.len() * 64,
            PreviewData::Binary { hex_dump, .. } => hex_dump.len(),
            PreviewData::Unsupported { .. } => 64,
        }
    }
}
