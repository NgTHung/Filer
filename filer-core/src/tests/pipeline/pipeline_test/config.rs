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
