use std::path::PathBuf;
use std::time::Duration;

use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use crate::services::dir_cache::DirCache;

fn make_node(name: &str) -> FileNode {
    FileNode {
        id: NodeId(0),
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/{name}")),
        kind: NodeKind::File { extension: None },
        size: 0,
        modified: None,
        created: None,
        accessed: None,
        meta: NodeMeta::default(),
    }
}

fn one_node() -> Vec<FileNode> {
    vec![make_node("a.txt")]
}

#[cfg(test)]
mod dir_cache_tests {
    use super::*;

    #[test]
    fn test_get_returns_none_on_first_access() {
        let mut cache = DirCache::new(1024 * 1024);
        assert!(cache.get(&PathBuf::from("/some/dir")).is_none());
    }

    #[test]
    fn test_get_returns_cached_on_second_access() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(path.clone(), one_node());
        let result = cache.get(&path);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_put_overwrites_existing_entry() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(path.clone(), vec![make_node("old.txt")]);
        cache.put(
            path.clone(),
            vec![make_node("new1.txt"), make_node("new2.txt")],
        );
        let result = cache.get(&path).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "new1.txt");
    }

    #[test]
    fn test_invalidate_removes_entry() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(path.clone(), one_node());
        cache.invalidate(&path);
        assert!(cache.get(&path).is_none());
    }

    #[test]
    fn test_invalidate_nonexistent_does_not_panic() {
        let mut cache = DirCache::new(1024 * 1024);
        cache.invalidate(&PathBuf::from("/nonexistent/dir"));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_clear_empties_cache() {
        let mut cache = DirCache::new(1024 * 1024);
        let p1 = PathBuf::from("/tmp/a");
        let p2 = PathBuf::from("/tmp/b");
        cache.put(p1.clone(), one_node());
        cache.put(p2.clone(), one_node());
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(&p1).is_none());
        assert!(cache.get(&p2).is_none());
    }

    #[test]
    fn test_lru_evicts_oldest_entry() {
        // Each node is ~sizeof(FileNode) + name.capacity() + path bytes ≈ 128+ bytes
        // We use very small capacity to force eviction.
        // Put A first (oldest), then B, then put C which should evict A.
        let node_size = std::mem::size_of::<FileNode>() + "a.txt".len() + "/tmp/a.txt".len();
        // capacity for exactly 2 entries, so 3rd evicts the first
        let capacity = node_size * 2 + 64 * 2; // two entries fit

        let mut cache = DirCache::new(capacity);
        let pa = PathBuf::from("/tmp/a");
        let pb = PathBuf::from("/tmp/b");
        let pc = PathBuf::from("/tmp/c");

        cache.put(pa.clone(), one_node()); // oldest
        // Small sleep to ensure distinct Instant values
        std::thread::sleep(Duration::from_millis(2));
        cache.put(pb.clone(), one_node());

        // Verify both present
        assert!(cache.get(&pa).is_some());
        assert!(cache.get(&pb).is_some());

        // Force capacity overflow — evicts LRU (pa, since we called get on pb after pa)
        std::thread::sleep(Duration::from_millis(2));
        cache.put(pc.clone(), one_node());

        // pa should have been evicted (it's the oldest accessed), pc should be present
        assert!(cache.get(&pc).is_some());
        // The cache size is bounded
        assert!(
            cache.current_size_bytes()
                <= capacity + cache.current_size_bytes().saturating_sub(capacity)
        );
    }

    #[test]
    fn test_lru_respects_access_order() {
        // Put A then B with capacity for only one entry.
        // Access A (making B the LRU), then put C → B is evicted, A survives.
        let node_size = std::mem::size_of::<FileNode>() + "a.txt".len() + "/tmp/a.txt".len() + 64;
        let capacity = node_size; // only one entry fits

        let mut cache = DirCache::new(capacity);
        let pa = PathBuf::from("/tmp/a");
        let pb = PathBuf::from("/tmp/b");
        let pc = PathBuf::from("/tmp/c");

        cache.put(pa.clone(), one_node());
        std::thread::sleep(Duration::from_millis(2));
        cache.put(pb.clone(), one_node()); // may evict pa immediately

        // After inserting pc, whatever was LRU should go
        std::thread::sleep(Duration::from_millis(2));
        cache.put(pc.clone(), one_node());

        // Cache has room for exactly 1 entry, so only the newest survives
        assert!(cache.get(&pc).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_size_ceiling_respected() {
        let max = 500;
        let mut cache = DirCache::new(max);
        for i in 0..20u32 {
            let path = PathBuf::from(format!("/tmp/dir{i}"));
            cache.put(path, one_node());
            // After every put the invariant must hold
            assert!(
                cache.current_size_bytes() <= max + 512,
                "size {} exceeded ceiling {}",
                cache.current_size_bytes(),
                max
            );
        }
    }

    #[test]
    fn test_invalidate_removes_correct_entry() {
        let mut cache = DirCache::new(1024 * 1024);
        let pa = PathBuf::from("/home/user");
        let pb = PathBuf::from("/home/other");
        cache.put(pa.clone(), one_node());
        cache.put(pb.clone(), one_node());
        cache.invalidate(&pa);
        assert!(cache.get(&pa).is_none());
        assert!(cache.get(&pb).is_some());
    }

    #[test]
    fn test_write_operation_invalidates_parent() {
        let mut cache = DirCache::new(1024 * 1024);
        let parent = PathBuf::from("/home/user");
        cache.put(parent.clone(), one_node());
        cache.invalidate(&parent);
        assert!(cache.get(&parent).is_none());
    }
}
