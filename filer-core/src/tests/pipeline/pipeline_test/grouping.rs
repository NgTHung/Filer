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

