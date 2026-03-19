//! SearchQuery parser unit tests
//!
//! Covers: plain text, filters (ext, size, type, date, name, regex),
//! options (case, hidden, depth, max), combined queries, and error cases.

use crate::model::query::{QueryFilter, SearchQuery};

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
        assert_eq!(
            q.filters,
            vec![QueryFilter::NameContains("config".into())]
        );
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
        let q =
            SearchQuery::parse("*.rs ext:rs size:>1kb type:file case:yes depth:5").unwrap();
        assert_eq!(q.text, "*.rs");
        assert!(q.filters.contains(&QueryFilter::Extension(vec!["rs".into()])));
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
