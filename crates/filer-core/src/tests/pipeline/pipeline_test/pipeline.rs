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

