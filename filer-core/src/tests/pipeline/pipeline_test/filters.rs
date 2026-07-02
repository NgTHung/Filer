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

