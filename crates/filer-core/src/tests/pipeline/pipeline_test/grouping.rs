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

#[test]
fn test_group_by_date_orders_by_time_group_sort_order() {
    let group = GroupBy::new(GroupField::Date);
    let now = SystemTime::now();
    let mut yesterday = make_file("yesterday.txt", 10, false);
    yesterday.modified = Some(now - Duration::from_secs(36 * 60 * 60));
    let mut older = make_file("older.txt", 20, false);
    older.modified = Some(now - Duration::from_secs(11 * 365 * 24 * 60 * 60));
    let mut recent = make_file("recent.txt", 30, false);
    recent.modified = Some(now - Duration::from_secs(5 * 60));

    let output = group.process(PipelineData::Flat(vec![yesterday, older, recent]));
    if let PipelineData::Grouped(groups) = output {
        assert_eq!(
            groups
                .groups
                .iter()
                .map(|group| group.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Last hour", "Yesterday", "Older"]
        );
        assert_eq!(
            groups
                .groups
                .iter()
                .map(|group| group.order)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    } else {
        panic!("Expected Grouped output");
    }
}

#[test]
fn test_group_by_size_orders_by_size_group_sort_order() {
    let group = GroupBy::new(GroupField::Size);

    let output = group.process(PipelineData::Flat(vec![
        make_file("huge.bin", 2 * 1024 * 1024 * 1024, false),
        make_file("empty.bin", 0, false),
        make_file("tiny.bin", 1024, false),
    ]));
    if let PipelineData::Grouped(groups) = output {
        assert_eq!(
            groups
                .groups
                .iter()
                .map(|group| group.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Empty", "Tiny (< 10 KB)", "Huge (1 GB - 10 GB)"]
        );
        assert_eq!(
            groups
                .groups
                .iter()
                .map(|group| group.order)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    } else {
        panic!("Expected Grouped output");
    }
}

