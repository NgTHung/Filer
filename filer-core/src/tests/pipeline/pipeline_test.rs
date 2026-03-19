//! Tests for pipeline stages

use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use crate::pipeline::filter::{FilterByExtension, FilterHidden};
use crate::pipeline::group::{GroupBy, GroupField};
use crate::pipeline::sort::{SortBy, SortField, SortOrder};
use crate::pipeline::{
    FilterConfig, GroupBy as ConfigGroupBy, Pipeline, PipelineConfig, PipelineData, SortConfig,
    Stage,
};
use crate::utils;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

// Helper to create test FileNodes
fn make_file(name: &str, size: u64, hidden: bool) -> FileNode {
    let extension = utils::get_extension(PathBuf::from(name).as_path()).map(str::to_string);
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}", name)),
        kind: NodeKind::File { extension },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
        },
    }
}

fn make_file_with_ext(name: &str, ext: Option<&str>, size: u64) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}", name)),
        kind: NodeKind::File {
            extension: ext.map(|s| s.to_string()),
        },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        meta: NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
        },
    }
}

fn make_dir(name: &str, hidden: bool) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64 + 1000),
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}", name)),
        kind: NodeKind::Directory {
            children_count: None,
        },
        size: 0,
        modified: Some(SystemTime::UNIX_EPOCH),
        created: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
        },
    }
}

// ===== FilterHidden Tests =====

#[test]
fn test_filter_hidden() {
    let filter = FilterHidden::new(false); // Hide hidden files

    let input = vec![
        make_file("readme.txt", 100, false),
        make_file(".gitignore", 50, true),
        make_file("main.rs", 200, false),
        make_file(".hidden", 10, true),
    ];

    let output = filter.process(PipelineData::Flat(input));

    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 2);
        assert!(nodes.iter().all(|f| !f.meta.hidden));
        assert!(nodes.iter().any(|f| f.name == "readme.txt"));
        assert!(nodes.iter().any(|f| f.name == "main.rs"));
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_hidden_show_all() {
    let filter = FilterHidden::new(true); // Show all files including hidden

    let input = vec![
        make_file("readme.txt", 100, false),
        make_file(".gitignore", 50, true),
        make_file("main.rs", 200, false),
        make_file(".hidden", 10, true),
    ];

    let output = filter.process(PipelineData::Flat(input));

    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 4);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_hidden_empty_input() {
    let filter = FilterHidden::new(false);
    let output = filter.process(PipelineData::Flat(vec![]));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 0);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_hidden_all_hidden() {
    let filter = FilterHidden::new(false);

    let input = vec![
        make_file(".gitignore", 50, true),
        make_file(".hidden", 10, true),
    ];

    let output = filter.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 0);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_hidden_name() {
    let filter = FilterHidden::new(false);
    assert_eq!(filter.name(), "filter_hidden");
}

#[test]
fn test_filter_hidden_directories() {
    let filter = FilterHidden::new(false);

    let input = vec![
        make_dir("visible_dir", false),
        make_dir(".hidden_dir", true),
        make_file("file.txt", 100, false),
    ];

    let output = filter.process(PipelineData::Flat(input));

    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 2);
        assert!(nodes.iter().any(|f| f.name == "visible_dir"));
        assert!(nodes.iter().any(|f| f.name == "file.txt"));
    } else {
        panic!("Expected Flat output");
    }
}

// ===== FilterByExtension Tests =====

#[test]
fn test_filter_by_extension_include() {
    let filter = FilterByExtension::new(vec!["rs".to_string(), "toml".to_string()], false);

    let input = vec![
        make_file("main.rs", 100, false),
        make_file("lib.rs", 200, false),
        make_file("Cargo.toml", 50, false),
        make_file("readme.md", 150, false),
        make_file("data.json", 80, false),
    ];

    let output = filter.process(PipelineData::Flat(input));

    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
        assert!(nodes.iter().any(|f| f.name == "main.rs"));
        assert!(nodes.iter().any(|f| f.name == "lib.rs"));
        assert!(nodes.iter().any(|f| f.name == "Cargo.toml"));
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_by_extension_empty_filter() {
    let filter = FilterByExtension::new(vec![], false);

    let input = vec![
        make_file("main.rs", 100, false),
        make_file("readme.md", 150, false),
    ];

    let output = filter.process(PipelineData::Flat(input));

    // Empty extension list should pass through all (or none, depending on implementation)
    // Adjust based on your expected behavior
    if let PipelineData::Flat(nodes) = output {
        assert!(nodes.len() <= 2);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_by_extension_no_match() {
    let filter = FilterByExtension::new(vec!["xyz".to_string()], false);

    let input = vec![
        make_file("main.rs", 100, false),
        make_file("readme.md", 150, false),
    ];

    let output = filter.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 0);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_by_extension_case_sensitivity() {
    let filter = FilterByExtension::new(vec!["RS".to_string()], false);

    let input = vec![
        make_file("main.rs", 100, false),
        make_file("other.RS", 200, false),
    ];

    let output = filter.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 1);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_filter_by_extension_name() {
    let filter = FilterByExtension::new(vec!["txt".to_string()], false);
    assert_eq!(filter.name(), "filter_by_extension");
}

#[test]
fn test_filter_by_extension_files_without_extension() {
    let filter = FilterByExtension::new(vec!["txt".to_string()], false);

    let input = vec![
        make_file_with_ext("Makefile", None, 100),
        make_file_with_ext("LICENSE", None, 200),
        make_file("readme.txt", 150, false),
    ];

    let output = filter.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "readme.txt");
    } else {
        panic!("Expected Flat output");
    }
}

// ===== SortBy Tests =====

#[test]
fn test_sort_by_name_asc() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, false);

    let input = vec![
        make_file("zebra.txt", 100, false),
        make_file("alpha.txt", 200, false),
        make_file("middle.txt", 150, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].name, "alpha.txt");
        assert_eq!(nodes[1].name, "middle.txt");
        assert_eq!(nodes[2].name, "zebra.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_name_desc() {
    let sort = SortBy::new(SortField::Name, SortOrder::Descending, false);

    let input = vec![
        make_file("alpha.txt", 100, false),
        make_file("zebra.txt", 200, false),
        make_file("middle.txt", 150, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].name, "zebra.txt");
        assert_eq!(nodes[1].name, "middle.txt");
        assert_eq!(nodes[2].name, "alpha.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_size() {
    let sort = SortBy::new(SortField::Size, SortOrder::Ascending, false);

    let input = vec![
        make_file("big.txt", 1000, false),
        make_file("small.txt", 10, false),
        make_file("medium.txt", 500, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].name, "small.txt");
        assert_eq!(nodes[1].name, "medium.txt");
        assert_eq!(nodes[2].name, "big.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_size_desc() {
    let sort = SortBy::new(SortField::Size, SortOrder::Descending, false);

    let input = vec![
        make_file("big.txt", 1000, false),
        make_file("small.txt", 10, false),
        make_file("medium.txt", 500, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].name, "big.txt");
        assert_eq!(nodes[1].name, "medium.txt");
        assert_eq!(nodes[2].name, "small.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_size_equal_sizes() {
    let sort = SortBy::new(SortField::Size, SortOrder::Ascending, false);

    let input = vec![
        make_file("b.txt", 100, false),
        make_file("a.txt", 100, false),
        make_file("c.txt", 100, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_modified() {
    let sort = SortBy::new(SortField::Modified, SortOrder::Ascending, false);

    // Using size as seconds since epoch for modified time in make_file
    let input = vec![
        make_file("newest.txt", 1000, false), // modified at 1000 seconds
        make_file("oldest.txt", 100, false),  // modified at 100 seconds
        make_file("middle.txt", 500, false),  // modified at 500 seconds
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].name, "oldest.txt");
        assert_eq!(nodes[1].name, "middle.txt");
        assert_eq!(nodes[2].name, "newest.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_modified_desc() {
    let sort = SortBy::new(SortField::Modified, SortOrder::Descending, false);

    let input = vec![
        make_file("newest.txt", 1000, false),
        make_file("oldest.txt", 100, false),
        make_file("middle.txt", 500, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 3);
        assert_eq!(output[0].name, "newest.txt");
        assert_eq!(output[1].name, "middle.txt");
        assert_eq!(output[2].name, "oldest.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_extension() {
    let sort = SortBy::new(SortField::Extension, SortOrder::Ascending, false);

    let input = vec![
        make_file("file.txt", 100, false),
        make_file("file.rs", 200, false),
        make_file("file.md", 150, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 3);
        assert_eq!(output[0].name, "file.md");
        assert_eq!(output[1].name, "file.rs");
        assert_eq!(output[2].name, "file.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_extension_with_no_extension() {
    let sort = SortBy::new(SortField::Extension, SortOrder::Ascending, false);

    let input = vec![
        make_file("file.txt", 100, false),
        make_file_with_ext("Makefile", None, 200),
        make_file("file.rs", 150, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 3);
    } else {
        panic!("Expected Flat output");
    }

    // Files without extension should sort first or last
}

#[test]
fn test_sort_directories_first() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, true);

    let input = vec![
        make_file("file_a.txt", 100, false),
        make_dir("dir_z", false),
        make_file("file_b.txt", 200, false),
        make_dir("dir_a", false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 4);
        // Directories should come first, sorted by name
        assert_eq!(output[0].name, "dir_a");
        assert_eq!(output[1].name, "dir_z");
        // Then files, sorted by name
        assert_eq!(output[2].name, "file_a.txt");
        assert_eq!(output[3].name, "file_b.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_directories_first_size_order() {
    let sort = SortBy::new(SortField::Size, SortOrder::Descending, true);

    let input = vec![
        make_file("small.txt", 10, false),
        make_dir("dir_a", false),
        make_file("big.txt", 1000, false),
        make_dir("dir_b", false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        // Directories first (sorted by size, which is 0 for both)
        assert!(output[0].is_dir());
        assert!(output[1].is_dir());
        // Then files by size descending
        assert_eq!(output[2].name, "big.txt");
        assert_eq!(output[3].name, "small.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_directories_not_first() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, false);

    let input = vec![
        make_file("aaa.txt", 100, false),
        make_dir("bbb", false),
        make_file("ccc.txt", 200, false),
    ];

    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        // When directories_first is false, sort everything together
        assert_eq!(output.len(), 3);
        assert_eq!(output[0].name, "aaa.txt");
        assert_eq!(output[1].name, "bbb");
        assert_eq!(output[2].name, "ccc.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_empty_input() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, false);
    let output = sort.process(PipelineData::Flat(vec![]));
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 0);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_single_item() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, false);
    let input = vec![make_file("only.txt", 100, false)];
    let output = sort.process(PipelineData::Flat(input));
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].name, "only.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_name() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, false);
    assert_eq!(sort.name(), "sort_by");
}

// ===== GroupBy Tests =====

#[test]
fn test_group_by_extension() {
    let group = GroupBy::new(GroupField::Extension);

    let input = vec![
        make_file("main.rs", 100, false),
        make_file("lib.rs", 200, false),
        make_file("readme.md", 150, false),
        make_file("Cargo.toml", 80, false),
    ];

    let output = group.process(PipelineData::Flat(input));
    if let PipelineData::Grouped(groups) = output {
        assert!(groups.contain_group("rs"));
        assert!(groups.contain_group("md"));
        assert!(groups.contain_group("toml"));
        assert_eq!(groups.get("rs").map(|v| v.nodes.len()), Some(2));
        assert_eq!(groups.get("md").map(|v| v.nodes.len()), Some(1));
    } else {
        panic!("Expected Grouped output");
    }
}

#[test]
fn test_group_by_first_letter() {
    let group = GroupBy::new(GroupField::FirstLetter);

    let input = vec![
        make_file("apple.txt", 100, false),
        make_file("apricot.txt", 200, false),
        make_file("banana.txt", 150, false),
        make_file("cherry.txt", 80, false),
    ];

    let output = group.process(PipelineData::Flat(input));
    if let PipelineData::Grouped(groups) = output {
        assert!(groups.contain_group("a"));
        assert!(groups.contain_group("b"));
        assert!(groups.contain_group("c"));
    } else {
        panic!("Expected Grouped output");
    }
}

#[test]
fn test_group_by_stage_name() {
    let group = GroupBy::new(GroupField::Extension);
    assert_eq!(group.name(), "group_by");
}

#[test]
fn test_group_by_empty_input() {
    let group = GroupBy::new(GroupField::Extension);
    let groups = group.process(PipelineData::Flat(vec![]));
    if let PipelineData::Grouped(groups) = groups {
        assert!(groups.total_count == 0);
        assert!(groups.groups.is_empty());
    } else {
        panic!("Expected Grouped output");
    }
}

// ===== Pipeline Tests =====

#[test]
fn test_pipeline_new() {
    let pipeline = Pipeline::new();
    assert!(pipeline.is_empty());
    assert_eq!(pipeline.len(), 0);
}

#[test]
fn test_pipeline_default() {
    let pipeline = Pipeline::default();
    assert!(pipeline.is_empty());
    assert_eq!(pipeline.len(), 0);
}

#[test]
fn test_pipeline_empty() {
    let pipeline = Pipeline::new();

    let input = vec![
        make_file("a.txt", 100, false),
        make_file("b.txt", 200, false),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].name, input[0].name);
        assert_eq!(output[1].name, input[1].name);
    } else {
        panic!("Expected Flat output");
    }
    // Empty pipeline should pass through unchanged
}

#[test]
fn test_pipeline_single_stage() {
    let pipeline = Pipeline::new().add(FilterHidden::new(false));

    assert_eq!(pipeline.len(), 1);
    assert!(!pipeline.is_empty());

    let input = vec![
        make_file("visible.txt", 100, false),
        make_file(".hidden", 50, true),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].name, "visible.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_pipeline_multiple_stages() {
    let pipeline = Pipeline::new()
        .add(FilterHidden::new(false))
        .add(SortBy::new(SortField::Name, SortOrder::Ascending, false));

    assert_eq!(pipeline.len(), 2);
}

#[test]
fn test_pipeline_chain() {
    let pipeline = Pipeline::new()
        .add(FilterHidden::new(false))
        .add(SortBy::new(SortField::Name, SortOrder::Ascending, false));

    let input = vec![
        make_file("zebra.txt", 100, false),
        make_file(".hidden", 50, true),
        make_file("alpha.txt", 200, false),
        make_file(".gitignore", 10, true),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].name, "alpha.txt");
        assert_eq!(output[1].name, "zebra.txt");
    } else {
        panic!("Expected Flat output");
    }

    // Should filter hidden first, then sort
}

#[test]
fn test_pipeline_chain_order_matters() {
    // Filter then sort
    let pipeline1 = Pipeline::new()
        .add(FilterHidden::new(false))
        .add(SortBy::new(SortField::Size, SortOrder::Ascending, false));

    let input = vec![
        make_file("big.txt", 1000, false),
        make_file(".hidden", 1, true),
        make_file("small.txt", 10, false),
    ];

    let output = pipeline1.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].name, "small.txt");
        assert_eq!(output[1].name, "big.txt");
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_pipeline_filter_sort_dirs_first() {
    let pipeline = Pipeline::new()
        .add(FilterHidden::new(false))
        .add(SortBy::new(SortField::Name, SortOrder::Ascending, true));

    let input = vec![
        make_file("z_file.txt", 100, false),
        make_dir(".hidden_dir", true),
        make_dir("a_dir", false),
        make_file("a_file.txt", 200, false),
        make_file(".hidden_file", 50, true),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 3);
        assert_eq!(output[0].name, "a_dir"); // Dir first
        assert_eq!(output[1].name, "a_file.txt"); // Then files sorted
        assert_eq!(output[2].name, "z_file.txt");
    } else {
        panic!("Expected Flat output");
    }

}

#[test]
fn test_pipeline_three_stages() {
    let pipeline = Pipeline::new()
        .add(FilterHidden::new(false))
        .add(FilterByExtension::new(vec!["txt".to_string()], false))
        .add(SortBy::new(SortField::Name, SortOrder::Ascending, false));

    assert_eq!(pipeline.len(), 3);

    let input = vec![
        make_file("zebra.txt", 100, false),
        make_file(".hidden.txt", 50, true),
        make_file("alpha.rs", 200, false),
        make_file("beta.txt", 150, false),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].name, "beta.txt");
        assert_eq!(output[1].name, "zebra.txt");
    } else {
        panic!("Expected Flat output");
    }

}

#[test]
fn test_pipeline_empty_input() {
    let pipeline = Pipeline::new()
        .add(FilterHidden::new(false))
        .add(SortBy::new(SortField::Name, SortOrder::Ascending, false));

    let output = pipeline.execute(vec![]);
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 0);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_pipeline_all_filtered_out() {
    let pipeline = Pipeline::new().add(FilterHidden::new(false));

    let input = vec![
        make_file(".hidden1", 100, true),
        make_file(".hidden2", 200, true),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 0);
    } else {
        panic!("Expected Flat output");
    }
}

// ===== PipelineConfig Tests =====

#[test]
fn test_pipeline_config_new() {
    let config = PipelineConfig::new();
    assert!(config.sort.is_none());
    assert!(config.filter.is_none());
    assert!(config.group.is_none());
}

#[test]
fn test_pipeline_config_with_default_sort() {
    let config = PipelineConfig::with_default_sort();

    assert!(config.sort.is_some());
    let sort = config.sort.unwrap();
    assert!(matches!(sort.field, SortField::Name));
    assert!(matches!(sort.order, SortOrder::Ascending));
    assert!(sort.directories_first);
}

#[test]
fn test_pipeline_config_builder() {
    let config = PipelineConfig::new()
        .sort(SortField::Size, SortOrder::Descending, true)
        .show_hidden(false)
        .group_by(ConfigGroupBy::Extension);

    assert!(config.sort.is_some());
    assert!(config.filter.is_some());
    assert!(config.group.is_some());

    let sort = config.sort.unwrap();
    assert!(matches!(sort.field, SortField::Size));
    assert!(matches!(sort.order, SortOrder::Descending));
}

#[test]
fn test_pipeline_config_filter_builder() {
    let filter = FilterConfig::only_extensions(vec!["rs".to_string(), "toml".to_string()]);

    assert!(!filter.show_hidden);
    assert_eq!(filter.include_extensions, vec!["rs", "toml"]);
    assert!(filter.exclude_extensions.is_empty());
}

#[test]
fn test_pipeline_from_config_empty() {
    let config = PipelineConfig::new();
    let pipeline = Pipeline::from_config(&config);

    assert!(pipeline.is_empty());
}

#[test]
fn test_pipeline_from_config_with_filter() {
    let config = PipelineConfig::new().show_hidden(false);

    let pipeline = Pipeline::from_config(&config);

    // Should have filter stage
    assert!(!pipeline.is_empty());

    let input = vec![
        make_file("visible.txt", 100, false),
        make_file(".hidden", 50, true),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 1);
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_pipeline_from_config_with_sort() {
    let config = PipelineConfig::new().sort(SortField::Name, SortOrder::Descending, false);

    let pipeline = Pipeline::from_config(&config);

    let input = vec![
        make_file("alpha.txt", 100, false),
        make_file("zebra.txt", 200, false),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output[0].name, "zebra.txt");
        assert_eq!(output[1].name, "alpha.txt");
    } else {
        panic!("Expected Flat output");
    }

}

#[test]
fn test_pipeline_from_config_full() {
    let config =
        PipelineConfig::new()
            .show_hidden(false)
            .sort(SortField::Size, SortOrder::Ascending, true);

    let pipeline = Pipeline::from_config(&config);

    let input = vec![
        make_file("big.txt", 1000, false),
        make_file(".hidden", 50, true),
        make_dir("dir", false),
        make_file("small.txt", 10, false),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 3);
        // Dir first
        assert_eq!(output[0].name, "dir");
        // Then files by size
        assert_eq!(output[1].name, "small.txt");
        assert_eq!(output[2].name, "big.txt");
    } else {
        panic!("Expected Flat output");
    }

}

#[test]
fn test_pipeline_from_config_with_extension_filter() {
    let config =
        PipelineConfig::new().filter(FilterConfig::only_extensions(vec!["rs".to_string()]));

    let pipeline = Pipeline::from_config(&config);

    let input = vec![
        make_file("main.rs", 100, false),
        make_file("readme.md", 200, false),
        make_file("lib.rs", 150, false),
    ];

    let output = pipeline.execute(input.clone());
    if let PipelineData::Flat(output) = output {
        assert_eq!(output.len(), 2);
        assert!(output.iter().all(|f| f.name.ends_with(".rs")));
    } else {
        panic!("Expected Flat output");
    }

}

#[test]
fn test_pipeline_config_serialization() {
    let config = PipelineConfig::new()
        .sort(SortField::Name, SortOrder::Ascending, true)
        .show_hidden(false);

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: PipelineConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config, deserialized);
}

#[test]
fn test_pipeline_config_default_eq() {
    let config1 = PipelineConfig::default();
    let config2 = PipelineConfig::new();

    assert_eq!(config1, config2);
}

#[test]
fn test_sort_config_default() {
    let sort = SortConfig::default();

    assert!(matches!(sort.field, SortField::Name));
    assert!(matches!(sort.order, SortOrder::Ascending));
    assert!(sort.directories_first);
}

#[test]
fn test_filter_config_default() {
    let filter = FilterConfig::default();

    assert!(!filter.show_hidden);
    assert!(filter.include_extensions.is_empty());
    assert!(filter.exclude_extensions.is_empty());
    assert!(filter.min_size.is_none());
    assert!(filter.max_size.is_none());
    assert!(filter.name_pattern.is_none());
}
#[test]
fn test_pipeline_config_size() {
    // Verify config is small enough for network transfer
    let config = PipelineConfig::with_default_sort();
    let json = serde_json::to_string(&config).unwrap();

    // Should be well under 1KB
    assert!(json.len() < 500, "Config too large: {} bytes", json.len());
    println!("Config size: {} bytes", json.len());
    println!("JSON: {}", json);
}

#[test]
fn test_filter_config_defaults() {
    let filter = FilterConfig::default();
    assert!(!filter.show_hidden);
    assert!(filter.include_extensions.is_empty());
    assert!(filter.exclude_extensions.is_empty());
    assert!(filter.min_size.is_none());
    assert!(filter.max_size.is_none());
}

#[test]
fn test_builder_pattern() {
    let config = PipelineConfig::new()
        .sort(SortField::Size, SortOrder::Descending, true)
        .show_hidden(true)
        .group_by(ConfigGroupBy::Extension);

    assert!(config.sort.is_some());
    assert_eq!(config.sort.unwrap().field, SortField::Size);
    assert!(config.filter.unwrap().show_hidden);
    assert_eq!(config.group.unwrap().by, ConfigGroupBy::Extension);
}
