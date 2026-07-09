//! SearchQuery parser unit tests
//!
//! Covers: plain text, filters (ext, size, type, date, name, regex),
//! options (case, hidden, depth, max), combined queries, and error cases.
//! Also covers SearchQuery::matches and QueryFilter::matches.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::model::node::{FileNode, NodeKind, NodeMeta};
use crate::model::query::{QueryFilter, SearchQuery};
use crate::tests::fixtures::local_file_node;

fn make_file(name: &str, size: u64) -> FileNode {
    let path = PathBuf::from("/test").join(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_string);
    local_file_node(
        path,
        name,
        NodeKind::File { extension: ext },
        size,
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    )
}

fn make_dir(name: &str) -> FileNode {
    let path = PathBuf::from("/test").join(name);
    local_file_node(
        path,
        name,
        NodeKind::Directory {
            children_count: None,
        },
        0,
        Some(SystemTime::UNIX_EPOCH),
        NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    )
}

fn make_hidden(name: &str) -> FileNode {
    let mut f = make_file(name, 100);
    f.meta.hidden = true;
    f
}

fn make_file_at(name: &str, size: u64, modified_secs: u64) -> FileNode {
    let mut f = make_file(name, size);
    f.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_secs));
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text() {
        let q = SearchQuery::parse("hello world").unwrap();
        assert_eq!(q.text, "hello world");
        assert!(q.filters.is_empty());
    }

    #[test]
    fn parse_empty_returns_error() {
        assert!(SearchQuery::parse("").is_err());
        assert!(SearchQuery::parse("   ").is_err());
    }

    #[test]
    fn parse_extension_filter() {
        let q = SearchQuery::parse("report ext:rs,py,toml").unwrap();
        assert_eq!(q.text, "report");
        assert_eq!(
            q.filters,
            vec![QueryFilter::Extension(vec![
                "rs".into(),
                "py".into(),
                "toml".into()
            ])]
        );
    }

    #[test]
    fn parse_extension_strips_dots() {
        let q = SearchQuery::parse("ext:.rs,.py").unwrap();
        assert_eq!(
            q.filters,
            vec![QueryFilter::Extension(vec!["rs".into(), "py".into()])]
        );
    }

    #[test]
    fn parse_size_filters() {
        let q = SearchQuery::parse("size:>1kb size:<10mb").unwrap();
        assert_eq!(q.filters.len(), 2);
        assert_eq!(q.filters[0], QueryFilter::SizeGreaterThan(1024));
        assert_eq!(q.filters[1], QueryFilter::SizeLessThan(10 * 1024 * 1024));
    }

    #[test]
    fn parse_size_gb() {
        let q = SearchQuery::parse("size:>2gb").unwrap();
        assert_eq!(
            q.filters[0],
            QueryFilter::SizeGreaterThan(2 * 1024 * 1024 * 1024)
        );
    }

    #[test]
    fn parse_size_plain_bytes() {
        let q = SearchQuery::parse("size:>4096").unwrap();
        assert_eq!(q.filters[0], QueryFilter::SizeGreaterThan(4096));
    }

    #[test]
    fn parse_type_file() {
        let q = SearchQuery::parse("type:file").unwrap();
        assert_eq!(q.filters, vec![QueryFilter::IsFile]);
    }

    #[test]
    fn parse_type_dir_aliases() {
        for alias in &["dir", "directory", "folder", "d"] {
            let q = SearchQuery::parse(&format!("type:{}", alias)).unwrap();
            assert_eq!(q.filters, vec![QueryFilter::IsDirectory]);
        }
    }

    #[test]
    fn parse_options() {
        let q = SearchQuery::parse("foo case:yes hidden:yes depth:3 max:50").unwrap();
        assert_eq!(q.text, "foo");
        assert!(q.options.case_sensitive);
        assert!(q.options.include_hidden);
        assert_eq!(q.options.max_depth, Some(3));
        assert_eq!(q.options.max_results, Some(50));
    }

    #[test]
    fn parse_hidden_adds_filter() {
        let q = SearchQuery::parse("hidden:yes").unwrap();
        assert!(q.options.include_hidden);
        assert!(q.filters.contains(&QueryFilter::IsHidden));
    }

    #[test]
    fn parse_name_contains() {
        let q = SearchQuery::parse("name:config").unwrap();
        assert_eq!(q.filters, vec![QueryFilter::NameContains("config".into())]);
    }

    #[test]
    fn parse_regex_match() {
        let q = SearchQuery::parse(r"match:^test_.*\.rs$").unwrap();
        assert_eq!(
            q.filters,
            vec![QueryFilter::NameMatches(r"^test_.*\.rs$".into())]
        );
    }

    #[test]
    fn parse_date_unix_timestamp() {
        let q = SearchQuery::parse("after:1700000000").unwrap();
        assert_eq!(q.filters, vec![QueryFilter::ModifiedAfter(1700000000)]);
    }

    #[test]
    fn parse_date_iso() {
        let q = SearchQuery::parse("after:2024-01-01 before:2024-12-31").unwrap();
        assert_eq!(q.filters.len(), 2);
        match &q.filters[0] {
            QueryFilter::ModifiedAfter(ts) => assert!(*ts > 0),
            other => panic!("Expected ModifiedAfter, got {:?}", other),
        }
        match &q.filters[1] {
            QueryFilter::ModifiedBefore(ts) => assert!(*ts > 0),
            other => panic!("Expected ModifiedBefore, got {:?}", other),
        }
    }

    #[test]
    fn parse_combined_query() {
        let q = SearchQuery::parse("*.rs ext:rs size:>1kb type:file case:yes depth:5").unwrap();
        assert_eq!(q.text, "*.rs");
        assert!(
            q.filters
                .contains(&QueryFilter::Extension(vec!["rs".into()]))
        );
        assert!(q.filters.contains(&QueryFilter::SizeGreaterThan(1024)));
        assert!(q.filters.contains(&QueryFilter::IsFile));
        assert!(q.options.case_sensitive);
        assert_eq!(q.options.max_depth, Some(5));
    }

    #[test]
    fn parse_only_filters_empty_text() {
        let q = SearchQuery::parse("ext:rs type:file").unwrap();
        assert_eq!(q.text, "");
        assert_eq!(q.filters.len(), 2);
    }

    #[test]
    fn parse_invalid_size_comparator() {
        assert!(SearchQuery::parse("size:=100").is_err());
    }

    #[test]
    fn parse_invalid_type() {
        assert!(SearchQuery::parse("type:symlink").is_err());
    }

    #[test]
    fn parse_invalid_bool() {
        assert!(SearchQuery::parse("case:maybe").is_err());
    }

    #[test]
    fn parse_invalid_depth() {
        assert!(SearchQuery::parse("depth:abc").is_err());
    }

    #[test]
    fn parse_invalid_date() {
        assert!(SearchQuery::parse("after:not-a-date").is_err());
    }

    #[test]
    fn parse_empty_ext_value() {
        assert!(SearchQuery::parse("ext:").is_err());
    }

    #[test]
    fn parse_empty_name_value() {
        assert!(SearchQuery::parse("name:").is_err());
    }

    #[test]
    fn parse_invalid_regex() {
        assert!(SearchQuery::parse("match:[invalid").is_err());
    }
}

#[cfg(test)]
mod matches_tests {
    use super::*;

    #[test]
    fn text_match_case_insensitive_by_default() {
        let q = SearchQuery::parse("README").unwrap();
        assert!(q.matches(&make_file("readme.md", 100)));
        assert!(q.matches(&make_file("README.txt", 100)));
        assert!(!q.matches(&make_file("other.txt", 100)));
    }

    #[test]
    fn text_match_case_sensitive() {
        let q = SearchQuery::parse("README case:yes").unwrap();
        assert!(q.matches(&make_file("README.txt", 100)));
        assert!(!q.matches(&make_file("readme.md", 100)));
    }

    #[test]
    fn empty_text_matches_any_name() {
        let q = SearchQuery::parse("type:file").unwrap();
        assert!(q.matches(&make_file("anything.xyz", 1)));
        assert!(!q.matches(&make_dir("a_dir")));
    }

    #[test]
    fn filter_extension_matches() {
        let q = SearchQuery::parse("ext:rs,toml").unwrap();
        assert!(q.matches(&make_file("main.rs", 100)));
        assert!(q.matches(&make_file("Cargo.toml", 50)));
        assert!(!q.matches(&make_file("readme.md", 50)));
    }

    #[test]
    fn filter_extension_case_insensitive_compare() {
        let q = SearchQuery::parse("ext:rs").unwrap();
        // ext stored lowercase, node extension extracted from path (lowercase on most FS)
        assert!(q.matches(&make_file("lib.rs", 100)));
        assert!(!q.matches(&make_file("lib.py", 100)));
    }

    #[test]
    fn filter_extension_no_match_on_directory() {
        let q = SearchQuery::parse("ext:rs").unwrap();
        assert!(!q.matches(&make_dir("src")));
    }

    #[test]
    fn filter_size_greater_than() {
        let q = SearchQuery::parse("size:>1000").unwrap();
        assert!(q.matches(&make_file("big.bin", 1001)));
        assert!(!q.matches(&make_file("small.bin", 1000)));
        assert!(!q.matches(&make_file("tiny.bin", 500)));
    }

    #[test]
    fn filter_size_less_than() {
        let q = SearchQuery::parse("size:<500").unwrap();
        assert!(q.matches(&make_file("small.txt", 499)));
        assert!(!q.matches(&make_file("medium.txt", 500)));
        assert!(!q.matches(&make_file("large.txt", 1000)));
    }

    #[test]
    fn filter_size_range() {
        let q = SearchQuery::parse("size:>100 size:<1000").unwrap();
        assert!(q.matches(&make_file("mid.txt", 500)));
        assert!(!q.matches(&make_file("tiny.txt", 50)));
        assert!(!q.matches(&make_file("huge.txt", 2000)));
    }

    #[test]
    fn filter_is_file() {
        let q = SearchQuery::parse("type:file").unwrap();
        assert!(q.matches(&make_file("doc.txt", 100)));
        assert!(!q.matches(&make_dir("docs")));
    }

    #[test]
    fn filter_is_directory() {
        let q = SearchQuery::parse("type:dir").unwrap();
        assert!(q.matches(&make_dir("src")));
        assert!(!q.matches(&make_file("src.txt", 100)));
    }

    #[test]
    fn filter_is_hidden() {
        let q = SearchQuery::parse("hidden:yes").unwrap();
        assert!(q.matches(&make_hidden(".env")));
        assert!(!q.matches(&make_file("visible.txt", 100)));
    }

    #[test]
    fn filter_name_contains() {
        let q = SearchQuery::parse("name:config").unwrap();
        assert!(q.matches(&make_file("app_config.toml", 100)));
        assert!(q.matches(&make_file("config.json", 100)));
        assert!(!q.matches(&make_file("readme.md", 100)));
    }

    #[test]
    fn filter_name_contains_is_case_sensitive() {
        // NameContains is always literal (case-sensitive substring)
        let q = SearchQuery::parse("name:Config").unwrap();
        assert!(!q.matches(&make_file("config.json", 100)));
        assert!(q.matches(&make_file("Config.json", 100)));
    }

    #[test]
    fn filter_name_matches_regex() {
        let q = SearchQuery::parse(r"match:^test_.*\.rs$").unwrap();
        assert!(q.matches(&make_file("test_search.rs", 100)));
        assert!(q.matches(&make_file("test_scan.rs", 200)));
        assert!(!q.matches(&make_file("main.rs", 300)));
        assert!(!q.matches(&make_file("test_nav.py", 400)));
    }

    #[test]
    fn filter_name_matches_anchored() {
        let q = SearchQuery::parse("match:^lib").unwrap();
        assert!(q.matches(&make_file("lib.rs", 100)));
        assert!(!q.matches(&make_file("stdlib.rs", 100)));
    }

    #[test]
    fn filter_modified_after() {
        let q = SearchQuery::parse("after:1700000000").unwrap();
        assert!(q.matches(&make_file_at("new.txt", 50, 2_000_000_000)));
        assert!(!q.matches(&make_file_at("old.txt", 50, 100)));
    }

    #[test]
    fn filter_modified_before() {
        let q = SearchQuery::parse("before:1700000000").unwrap();
        assert!(q.matches(&make_file_at("old.txt", 50, 100)));
        assert!(!q.matches(&make_file_at("new.txt", 50, 2_000_000_000)));
    }

    #[test]
    fn filter_modified_none_fails() {
        // node with no modified time fails date filters
        let q = SearchQuery::parse("after:0").unwrap();
        let mut f = make_file("no_time.txt", 100);
        f.modified = None;
        assert!(!q.matches(&f));
    }

    #[test]
    fn multiple_filters_all_must_pass() {
        let q = SearchQuery::parse("ext:rs size:>100").unwrap();
        assert!(q.matches(&make_file("big.rs", 200)));
        assert!(!q.matches(&make_file("small.rs", 50))); // size fails
        assert!(!q.matches(&make_file("big.py", 200))); // ext fails
    }

    #[test]
    fn text_and_filter_both_must_pass() {
        let q = SearchQuery::parse("test ext:rs").unwrap();
        assert!(q.matches(&make_file("test_main.rs", 100)));
        assert!(!q.matches(&make_file("test_main.py", 100))); // ext fails
        assert!(!q.matches(&make_file("main.rs", 100))); // text fails
    }

    #[test]
    fn query_filter_extension_direct() {
        let f = QueryFilter::Extension(vec!["rs".into()]);
        assert!(f.matches(&make_file("main.rs", 0)));
        assert!(!f.matches(&make_file("main.py", 0)));
    }

    #[test]
    fn query_filter_size_direct() {
        let gt = QueryFilter::SizeGreaterThan(100);
        let lt = QueryFilter::SizeLessThan(100);
        assert!(gt.matches(&make_file("f", 101)));
        assert!(!gt.matches(&make_file("f", 100)));
        assert!(lt.matches(&make_file("f", 99)));
        assert!(!lt.matches(&make_file("f", 100)));
    }
}
