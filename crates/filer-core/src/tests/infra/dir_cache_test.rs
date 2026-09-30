use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::model::location::{Location, LocationDescriptor, LocationId};
use crate::model::node::{NodeEntry, NodeKind, NodeMeta};
use crate::services::dir_cache::DirCache;
use crate::tests::fixtures::local_file_node;
use crate::vfs::provider::ListingOptions;

fn make_node(name: &str) -> NodeEntry {
    let path = PathBuf::from(format!("/tmp/{name}"));
    local_file_node(
        path,
        name,
        NodeKind::File { extension: None },
        0,
        None,
        NodeMeta::default(),
    )
}

fn one_node() -> Vec<NodeEntry> {
    vec![make_node("a.txt")]
}

fn location(path: &Path) -> Location {
    Location::local(path.to_path_buf())
}

fn location_id(path: &Path) -> LocationId {
    location(path).id()
}

#[cfg(test)]
mod dir_cache_tests {
    use super::*;

    fn fast() -> ListingOptions {
        ListingOptions::fast()
    }

    fn metadata() -> ListingOptions {
        ListingOptions::metadata()
    }

    fn entry_size(path: &Path) -> usize {
        let mut probe = DirCache::new(usize::MAX);
        probe.put(location(path), fast(), one_node());
        probe.current_size_bytes()
    }

    #[test]
    fn test_get_returns_none_on_first_access() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/some/dir");
        assert!(cache.get(location_id(&path), fast()).is_none());
    }

    #[test]
    fn test_get_returns_cached_on_second_access() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(location(&path), fast(), one_node());
        let result = cache.get(location_id(&path), fast());
        assert!(result.is_some());
        assert_eq!(result.map(|entries| entries.len()), Some(1));
    }

    #[test]
    fn test_put_overwrites_existing_entry() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(location(&path), fast(), vec![make_node("old.txt")]);
        cache.put(
            location(&path),
            fast(),
            vec![make_node("new1.txt"), make_node("new2.txt")],
        );
        let result = cache.get(location_id(&path), fast()).expect("cache hit");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "new1.txt");
    }

    #[test]
    fn test_invalidate_removes_entry() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(location(&path), fast(), one_node());
        cache.invalidate(location_id(&path));
        assert!(cache.get(location_id(&path), fast()).is_none());
    }

    #[test]
    fn test_invalidate_nonexistent_does_not_panic() {
        let mut cache = DirCache::new(1024 * 1024);
        cache.invalidate(location_id(Path::new("/nonexistent/dir")));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_clear_empties_cache() {
        let mut cache = DirCache::new(1024 * 1024);
        let p1 = PathBuf::from("/tmp/a");
        let p2 = PathBuf::from("/tmp/b");
        cache.put(location(&p1), fast(), one_node());
        cache.put(location(&p2), fast(), one_node());
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(location_id(&p1), fast()).is_none());
        assert!(cache.get(location_id(&p2), fast()).is_none());
    }

    #[test]
    fn test_lru_evicts_oldest_entry() {
        let sample_size = entry_size(Path::new("/tmp/a"));
        let capacity = sample_size * 2 + sample_size / 2;
        let mut cache = DirCache::new(capacity);
        let pa = PathBuf::from("/tmp/a");
        let pb = PathBuf::from("/tmp/b");
        let pc = PathBuf::from("/tmp/c");

        cache.put(location(&pa), fast(), one_node());
        std::thread::sleep(Duration::from_millis(2));
        cache.put(location(&pb), fast(), one_node());
        assert!(cache.get(location_id(&pa), fast()).is_some());
        assert!(cache.get(location_id(&pb), fast()).is_some());

        std::thread::sleep(Duration::from_millis(2));
        cache.put(location(&pc), fast(), one_node());

        assert!(cache.get(location_id(&pa), fast()).is_none());
        assert!(cache.get(location_id(&pc), fast()).is_some());
        assert!(cache.current_size_bytes() <= capacity + sample_size);
    }

    #[test]
    fn test_lru_respects_access_order() {
        let sample_size = entry_size(Path::new("/tmp/a"));
        let capacity = sample_size * 2 + sample_size / 2;
        let mut cache = DirCache::new(capacity);
        let pa = PathBuf::from("/tmp/a");
        let pb = PathBuf::from("/tmp/b");
        let pc = PathBuf::from("/tmp/c");

        cache.put(location(&pa), fast(), one_node());
        std::thread::sleep(Duration::from_millis(2));
        cache.put(location(&pb), fast(), one_node());
        assert!(cache.get(location_id(&pa), fast()).is_some());
        std::thread::sleep(Duration::from_millis(2));
        cache.put(location(&pc), fast(), one_node());

        assert!(cache.get(location_id(&pa), fast()).is_some());
        assert!(cache.get(location_id(&pb), fast()).is_none());
        assert!(cache.get(location_id(&pc), fast()).is_some());
    }

    #[test]
    fn test_size_ceiling_respected() {
        let sample_size = entry_size(Path::new("/tmp/dir0"));
        let max = sample_size * 2;
        let mut cache = DirCache::new(max);
        for i in 0..20u32 {
            let path = PathBuf::from(format!("/tmp/dir{i}"));
            cache.put(location(&path), fast(), one_node());
            assert!(
                cache.current_size_bytes() <= max + sample_size,
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
        cache.put(location(&pa), fast(), one_node());
        cache.put(location(&pb), fast(), one_node());
        cache.invalidate(location_id(&pa));
        assert!(cache.get(location_id(&pa), fast()).is_none());
        assert!(cache.get(location_id(&pb), fast()).is_some());
    }

    #[test]
    fn test_write_operation_invalidates_parent() {
        let mut cache = DirCache::new(1024 * 1024);
        let parent = PathBuf::from("/home/user");
        cache.put(location(&parent), fast(), one_node());
        cache.invalidate(location_id(&parent));
        assert!(cache.get(location_id(&parent), fast()).is_none());
    }

    #[test]
    fn test_listing_detail_has_separate_cache_entries() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(location(&path), fast(), vec![make_node("fast.txt")]);
        cache.put(location(&path), metadata(), vec![make_node("metadata.txt")]);

        assert_eq!(
            cache.get(location_id(&path), fast()).expect("fast hit")[0].name,
            "fast.txt"
        );
        assert_eq!(
            cache
                .get(location_id(&path), metadata())
                .expect("metadata hit")[0]
                .name,
            "metadata.txt"
        );
    }

    #[test]
    fn test_invalidate_removes_all_listing_detail_entries_for_location() {
        let mut cache = DirCache::new(1024 * 1024);
        let path = PathBuf::from("/tmp/dir");
        cache.put(location(&path), fast(), one_node());
        cache.put(location(&path), metadata(), one_node());

        cache.invalidate(location_id(&path));

        assert!(cache.get(location_id(&path), fast()).is_none());
        assert!(cache.get(location_id(&path), metadata()).is_none());
    }

    #[test]
    fn test_invalidate_local_subtree_removes_exact_path_and_descendants() {
        let mut cache = DirCache::new(1024 * 1024);
        let root = PathBuf::from("/tmp/project");
        let child = PathBuf::from("/tmp/project/src");
        let grandchild = PathBuf::from("/tmp/project/src/bin");
        let sibling = PathBuf::from("/tmp/project-other");

        cache.put(location(&root), fast(), one_node());
        cache.put(location(&child), fast(), one_node());
        cache.put(location(&grandchild), fast(), one_node());
        cache.put(location(&sibling), fast(), one_node());

        cache.invalidate_local_subtree(&root);

        assert!(cache.get(location_id(&root), fast()).is_none());
        assert!(cache.get(location_id(&child), fast()).is_none());
        assert!(cache.get(location_id(&grandchild), fast()).is_none());
        assert!(cache.get(location_id(&sibling), fast()).is_some());
    }

    #[test]
    fn test_provider_locations_with_same_path_remain_distinct() {
        let mut cache = DirCache::new(1024 * 1024);
        let local = Location::local("/tmp/shared");
        let remote = Location::new(LocationDescriptor::provider_profile(
            "s3",
            "default",
            "/tmp/shared",
        ));

        cache.put(local.clone(), fast(), vec![make_node("local.txt")]);
        cache.put(remote.clone(), fast(), vec![make_node("remote.txt")]);

        assert_eq!(cache.len(), 2);
        assert_eq!(
            cache.get(local.id(), fast()).expect("local hit")[0].name,
            "local.txt"
        );
        assert_eq!(
            cache.get(remote.id(), fast()).expect("remote hit")[0].name,
            "remote.txt"
        );

        cache.invalidate(local.id());
        assert!(cache.get(local.id(), fast()).is_none());
        assert!(cache.get(remote.id(), fast()).is_some());
    }

    #[test]
    fn test_local_subtree_invalidation_does_not_remove_nonlocal_locations() {
        let mut cache = DirCache::new(1024 * 1024);
        let remote = Location::new(LocationDescriptor::provider_profile(
            "s3",
            "default",
            "/tmp/project/src",
        ));
        cache.put(remote.clone(), fast(), one_node());

        cache.invalidate_local_subtree(Path::new("/tmp/project"));

        assert!(cache.get(remote.id(), fast()).is_some());
    }
}
