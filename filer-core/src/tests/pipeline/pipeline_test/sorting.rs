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
        assert_eq!(
            nodes.iter().map(|node| node.name.as_str()).collect::<Vec<_>>(),
            vec!["a.txt", "b.txt", "c.txt"]
        );
    } else {
        panic!("Expected Flat output");
    }
}

#[test]
fn test_sort_by_size_ties_by_path_after_name() {
    let sort = SortBy::new(SortField::Size, SortOrder::Ascending, false);
    let mut later = make_file("same.txt", 100, false);
    later.path = PathBuf::from("/test/b/same.txt");
    let mut earlier = make_file("same.txt", 100, false);
    earlier.path = PathBuf::from("/test/a/same.txt");

    let output = sort.process(PipelineData::Flat(vec![later, earlier]));
    if let PipelineData::Flat(nodes) = output {
        assert_eq!(
            nodes
                .iter()
                .map(|node| node.path.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["/test/a/same.txt", "/test/b/same.txt"]
        );
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
        assert_eq!(
            output
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Makefile", "file.rs", "file.txt"]
        );
    } else {
        panic!("Expected Flat output");
    }
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

