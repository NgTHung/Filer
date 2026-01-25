//! Tests for utility functions

mod path_tests {
    use crate::utils::path::*;
    use std::path::Path;

    #[test]
    fn test_get_extension() {
        let test_path = Path::new("hi.txt");
        assert_eq!(get_extension(test_path), Some("txt"));
        let test_path = Path::new("some/path/../travelling///o.js");
        assert_eq!(get_extension(test_path), Some("js"));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        assert_eq!(get_extension(test_path), Some("type"));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another.type");
        assert_ne!(get_extension(test_path), None);
        let test_path = Path::new("wierd.ash.file.type");
        assert_eq!(get_extension(test_path), Some("type"));
        let test_path = Path::new("host.kq");
        assert_ne!(get_extension(test_path), None);
    }

    #[test]
    fn test_get_extension_none() {
        let test_path = Path::new("hi");
        assert_eq!(get_extension(test_path), None);
        let test_path = Path::new("some/path/../travelling///o");
        assert_eq!(get_extension(test_path), None);
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        assert_ne!(get_extension(test_path), None);
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another");
        assert_eq!(get_extension(test_path), None);
        let test_path = Path::new("wierd.ash.file.type");
        assert_ne!(get_extension(test_path), None);
        let test_path = Path::new("host.kq");
        assert_ne!(get_extension(test_path), None);
    }

    #[test]
    fn test_get_stem() {
        let test_path = Path::new("hi");
        assert_eq!(get_stem(test_path), Some("hi"));
        let test_path = Path::new("some/path/../travelling///.o.js");
        assert_eq!(get_stem(test_path), Some(".o"));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        assert_eq!(get_stem(test_path), Some("wierd.ash.file"));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another");
        assert_eq!(get_stem(test_path), Some("another"));
        let test_path = Path::new("wierd.ash.file.type");
        assert_eq!(get_stem(test_path), Some("wierd.ash.file"));
        let test_path = Path::new("host.kq");
        assert_eq!(get_stem(test_path), Some("host"));
        let test_path = Path::new(".k");
        assert_eq!(get_stem(test_path), Some(".k"));
        let test_path = Path::new("/");
        assert_eq!(get_stem(test_path), None);
    }

    #[test]
    fn test_is_hidden_unix() {
        let test_path = Path::new("hi");
        assert_eq!(is_hidden(test_path), false);
        let test_path = Path::new("some/path/../travelling///.o.js");
        assert_eq!(is_hidden(test_path), true);
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        assert_eq!(is_hidden(test_path), false);
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another");
        assert_eq!(is_hidden(test_path), false);
        let test_path = Path::new(".wierd.ash.file.type");
        assert_eq!(is_hidden(test_path), true);
        let test_path = Path::new("host.kq");
        assert_eq!(is_hidden(test_path), false);
        let test_path = Path::new("/home/kalasana/.bbq/test/another");
        assert_eq!(is_hidden(test_path), true);
        let test_path = Path::new("/");
        assert_eq!(is_hidden(test_path), false);
    }

    #[test]
    fn test_parent_name() {
        let test_path = Path::new("hi");
        assert_eq!(parent_name(test_path), Some(""));
        let test_path = Path::new("some/path/../travelling///.o.js");
        assert_eq!(parent_name(test_path), Some("some/path/../travelling"));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        assert_eq!(parent_name(test_path), Some("some/path/../travelling"));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another");
        assert_eq!(
            parent_name(test_path),
            Some("some/path/../travelling///wierd.ash.file.type")
        );
        let test_path = Path::new("wierd.ash.file.type");
        assert_eq!(parent_name(test_path), Some(""));
        let test_path = Path::new("../../");
        assert_eq!(parent_name(test_path), Some(".."));
        let test_path = Path::new("/hi/bbq");
        assert_eq!(parent_name(test_path), Some("/hi"));
        let test_path = Path::new("/");
        assert_eq!(parent_name(test_path), None);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_normalize_sep() {
        use std::path::PathBuf;

        let test_path = Path::new("hi");
        let res = PathBuf::from("hi");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("some/path/../travelling///.o.js");
        let res = PathBuf::from("some/travelling/.o.js");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        let res = PathBuf::from("some/travelling/wierd.ash.file.type");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another");
        let res = PathBuf::from("some/travelling/wierd.ash.file.type/another");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("wierd.ash.file.type");
        let res = PathBuf::from("wierd.ash.file.type");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("../../");
        let res = PathBuf::from("/");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("/hi/bbq");
        let res = PathBuf::from("/hi/bbq");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("/");
        let res = PathBuf::from("/");
        assert!(matches!(normalize(test_path), Ok(res)));
    }
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_normalize_sep() {
        use std::path::PathBuf;

        let test_path = Path::new("hi");
        let res = PathBuf::from("hi");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("some/path/../travelling///.o.js");
        let res = PathBuf::from("some/path/../travelling/.o.js");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type");
        let res = PathBuf::from("some/path/../travelling/wierd.ash.file.type");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("some/path/../travelling///wierd.ash.file.type/another");
        let res = PathBuf::from("some/path/../travelling/wierd.ash.file.type/another");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("wierd.ash.file.type");
        let res = PathBuf::from("wierd.ash.file.type");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("../../");
        let res = PathBuf::from("../../");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("/hi/bbq");
        let res = PathBuf::from("/hi/bbq");
        assert!(matches!(normalize(test_path), Ok(res)));
        let test_path = Path::new("/");
        let res = PathBuf::from("/");
        assert!(matches!(normalize(test_path), Ok(res)));
    }
}

mod size_tests {
    use crate::utils::size::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(255), "255 B");
    }

    #[test]
    fn test_format_kilobytes() {
        assert_eq!(format_size(512), "0.5 KB");
        assert_eq!(format_size(768), "0.75 KB");
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(2048), "2 KB");
        assert_eq!(format_size(1024 * 100), "100 KB");
    }

    #[test]
    fn test_format_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1 MB");
        assert_eq!(format_size(1024 * 1024 + 512 * 1024), "1.5 MB");
        assert_eq!(format_size(1024 * 1024 * 10), "10 MB");
        assert_eq!(format_size(1024 * 1024 * 100), "100 MB");
    }

    #[test]
    fn test_format_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 512), "0.5 GB");
        assert_eq!(format_size(1024 * 1024 * 1024 * 2), "2 GB");
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 1024), "1 TB");
        assert_eq!(format_size(1324997410), "1.234 GB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100"), Some(100));
        assert_eq!(parse_size("100 B"), Some(100));
        assert_eq!(parse_size("1 KB"), Some(1024));
        assert_eq!(parse_size("1.5 KB"), Some(1536));
        assert_eq!(parse_size("1 MB"), Some(1024 * 1024));
        assert_eq!(parse_size("1.5 MB"), Some(1024 * 1024 + 512 * 1024));
        assert_eq!(parse_size("1 GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("2GB"), Some(1024 * 1024 * 1024 * 2));
        assert_eq!(parse_size("1TB"), Some(1024u64 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(parse_size(""), None);
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("1.5.5 KB"), None);
        assert_eq!(parse_size("KB"), None);
        assert_eq!(parse_size("1 XB"), None);
    }

    #[test]
    fn test_size_unit_multiplier() {
        assert_eq!(SizeUnit::Bytes.multiplier(), 1);
        assert_eq!(SizeUnit::Kilobytes.multiplier(), 1024);
        assert_eq!(SizeUnit::Megabytes.multiplier(), 1024 * 1024);
        assert_eq!(SizeUnit::Gigabytes.multiplier(), 1024 * 1024 * 1024);
        assert_eq!(SizeUnit::Terabytes.multiplier(), 1024u64 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_size_unit_abbrev() {
        assert_eq!(SizeUnit::Bytes.abbrev(), "B");
        assert_eq!(SizeUnit::Kilobytes.abbrev(), "KB");
        assert_eq!(SizeUnit::Megabytes.abbrev(), "MB");
        assert_eq!(SizeUnit::Gigabytes.abbrev(), "GB");
        assert_eq!(SizeUnit::Terabytes.abbrev(), "TB");
    }
}

mod time_tests {
    use crate::utils::time::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(1.0), "0:01");
        assert_eq!(format_duration(30.0), "0:30");
        assert_eq!(format_duration(59.0), "0:59");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(60.0), "1:00");
        assert_eq!(format_duration(90.0), "1:30");
        assert_eq!(format_duration(125.0), "2:05");
        assert_eq!(format_duration(3599.0), "59:59");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3600.0), "1:00:00");
        assert_eq!(format_duration(3661.0), "1:01:01");
        assert_eq!(format_duration(5025.0), "1:23:45");
        assert_eq!(format_duration(86400.0), "24:00:00");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("0:00"), Some(0.0));
        assert_eq!(parse_duration("0:01"), Some(1.0));
        assert_eq!(parse_duration("0:30"), Some(30.0));
        assert_eq!(parse_duration("1:00"), Some(60.0));
        assert_eq!(parse_duration("1:30"), Some(90.0));
        assert_eq!(parse_duration("2:05"), Some(125.0));
        assert_eq!(parse_duration("1:00:00"), Some(3600.0));
        assert_eq!(parse_duration("1:01:01"), Some(3661.0));
        assert_eq!(parse_duration("1:23:45"), Some(5025.0));
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("1:2:3:4"), None);
        assert_eq!(parse_duration("1.5"), None);
    }

    #[test]
    fn test_format_time() {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000000000);
        let formatted = format_time(time);
        // Basic check that it returns a non-empty string
        assert!(!formatted.is_empty());
    }

    #[test]
    fn test_format_relative() {
        let now = SystemTime::now();
        let past = now - Duration::from_secs(3600);
        let relative = format_relative(past);
        // Basic check that it returns a non-empty string
        assert!(!relative.is_empty());
    }
}
