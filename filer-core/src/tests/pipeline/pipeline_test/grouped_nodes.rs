#[test]
fn test_grouped_nodes_unbounded_limit_preserves_all_rows() {
    let grouped = grouped_nodes(vec![(
        "",
        vec![make_file("a.txt", 1, false), make_file("b.txt", 2, false)],
    )]);

    let (limited, load) = grouped.limited(None);

    assert_eq!(limited.total_count, 2);
    assert_eq!(limited.groups[0].nodes.len(), 2);
    assert_eq!(load.loaded_count, 2);
    assert_eq!(load.total_count, Some(2));
    assert!(load.complete);
}

#[test]
fn test_grouped_nodes_bounded_limit_trims_across_groups() {
    let grouped = grouped_nodes(vec![
        (
            "a",
            vec![make_file("a1.txt", 1, false), make_file("a2.txt", 2, false)],
        ),
        (
            "b",
            vec![make_file("b1.txt", 3, false), make_file("b2.txt", 4, false)],
        ),
    ]);

    let (limited, load) = grouped.limited(Some(3));

    assert_eq!(limited.total_count, 3);
    assert_eq!(limited.groups.len(), 2);
    assert_eq!(limited.groups[0].nodes.len(), 2);
    assert_eq!(limited.groups[1].nodes.len(), 1);
    assert_eq!(limited.groups[1].nodes[0].name, "b1.txt");
    assert_eq!(load.loaded_count, 3);
    assert_eq!(load.total_count, Some(4));
    assert!(!load.complete);
}

#[test]
fn test_grouped_nodes_bounded_zero_returns_empty_complete_only_for_empty_total() {
    let grouped = grouped_nodes(vec![("a", vec![make_file("a.txt", 1, false)])]);
    let (limited, load) = grouped.limited(Some(0));

    assert_eq!(limited.total_count, 0);
    assert!(limited.groups.is_empty());
    assert_eq!(load.loaded_count, 0);
    assert_eq!(load.total_count, Some(1));
    assert!(!load.complete);

    let (empty_limited, empty_load) = grouped_nodes(vec![]).limited(Some(0));
    assert_eq!(empty_limited.total_count, 0);
    assert!(empty_load.complete);
}

