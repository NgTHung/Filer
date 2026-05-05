use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::model::node::FileNode;

pub type SharedDirCache = Arc<Mutex<DirCache>>;

struct CacheEntry {
    nodes: Vec<FileNode>,
    size_bytes: usize,
    accessed: Instant,
}

/// LRU directory listing cache shared between Scanner and Operator.
///
/// - `get` returns a clone of the cached nodes, updating `accessed` for LRU.
/// - `put` inserts (or replaces) an entry, evicting LRU entries until the
///   total size fits within `max_size_bytes`.
/// - `invalidate` removes a single path entry.
/// - `clear` empties the cache entirely.
///
/// Size is estimated as `sizeof(FileNode) + name.capacity() + path bytes` per
/// node, with a minimum of 64 bytes per entry.
pub struct DirCache {
    entries: HashMap<PathBuf, CacheEntry>,
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

    /// Look up a directory listing. Updates `accessed` on hit.
    pub fn get(&mut self, path: &Path) -> Option<Vec<FileNode>> {
        let entry = self.entries.get_mut(path)?;
        entry.accessed = Instant::now();
        Some(entry.nodes.clone())
    }

    /// Insert or replace a directory listing, evicting LRU entries as needed.
    pub fn put(&mut self, path: PathBuf, nodes: Vec<FileNode>) {
        let new_size = estimate_size(&nodes);

        // Remove existing entry for this path first (adjust size).
        if let Some(old) = self.entries.remove(&path) {
            self.current_size_bytes -= old.size_bytes;
        }

        // Evict LRU entries until the new entry fits.
        while !self.entries.is_empty() && self.current_size_bytes + new_size > self.max_size_bytes {
            self.evict_lru();
        }

        self.current_size_bytes += new_size;
        self.entries.insert(
            path,
            CacheEntry {
                nodes,
                size_bytes: new_size,
                accessed: Instant::now(),
            },
        );
    }

    /// Remove the entry for `path` (no-op if not present).
    pub fn invalidate(&mut self, path: &Path) {
        if let Some(old) = self.entries.remove(path) {
            self.current_size_bytes -= old.size_bytes;
        }
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size_bytes = 0;
    }

    /// Number of cached directories.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Aggregate size of all cached entries in bytes.
    pub fn current_size_bytes(&self) -> usize {
        self.current_size_bytes
    }

    /// Remove the least-recently-accessed entry.
    fn evict_lru(&mut self) {
        let oldest_key = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.accessed)
            .map(|(k, _)| k.clone());

        if let Some(key) = oldest_key {
            if let Some(evicted) = self.entries.remove(&key) {
                self.current_size_bytes -= evicted.size_bytes;
            }
        }
    }
}

fn estimate_size(nodes: &[FileNode]) -> usize {
    nodes
        .iter()
        .map(|n| std::mem::size_of::<FileNode>() + n.name.capacity() + n.path.as_os_str().len())
        .sum::<usize>()
        .max(64)
}
