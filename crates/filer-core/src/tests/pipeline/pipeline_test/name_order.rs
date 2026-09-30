use crate::pipeline::name_order::{compare_names, name_key};
use std::cmp::Ordering;

/// Which step of the name order decides an ADR 0002 example pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecidingStep {
    MainKey,
    LeadingZeros,
    RawBytes,
}

/// Every example pair in docs/adr/0002-default-name-order.md, earlier first.
const ADR_EXAMPLES: &[(&str, &str, DecidingStep)] = &[
    ("alpha", "Zeta", DecidingStep::MainKey),
    ("File1", "file1", DecidingStep::RawBytes),
    ("_build", "alpha", DecidingStep::MainKey),
    ("file2", "file10", DecidingStep::MainKey),
    ("IMG_0010", "IMG_100", DecidingStep::MainKey),
    ("file.txt", "file1.txt", DecidingStep::MainKey),
    ("file1.txt", "file_1.txt", DecidingStep::MainKey),
    ("file1", "file01", DecidingStep::LeadingZeros),
    ("file1a", "file01b", DecidingStep::MainKey),
    ("Été", "été", DecidingStep::RawBytes),
    ("Tài liệu 2", "tài liệu 10", DecidingStep::MainKey),
    ("zebra", "Ärger", DecidingStep::MainKey),
];

/// Names that stress case, digit runs, leading zeros, and non-ASCII folding.
const EDGE_NAMES: &[&str] = &[
    "",
    "a",
    "A",
    "_build",
    "alpha",
    "Alpha",
    "ALPHA",
    "Zeta",
    "zebra",
    "file",
    "file.txt",
    "file-2.txt",
    "file_1",
    "file 1",
    "file1",
    "File1",
    "file01",
    "FILE01",
    "file001",
    "file1a",
    "file01b",
    "file2",
    "file10",
    "x9",
    "x9y",
    "x09y",
    "0",
    "00",
    "000a",
    "IMG_0009",
    "IMG_0010",
    "IMG_100",
    "Ärger",
    "ärger",
    "Été",
    "été",
    "\u{212A}elvin",
    "kelvin",
    "Kelvin",
    "\u{130}stanbul",
    "i\u{307}stanbul",
    "istanbul",
    "Straße",
    "strasse",
    "STRASSE",
    "ß",
    "ss",
    "Tài liệu 2",
    "tài liệu 10",
    "写真 1",
    "12345678901234567890123",
    "12345678901234567890124",
    "99999999999999999999",
    "100000000000000000000",
];

#[test]
fn name_order_matches_every_adr_example_at_its_deciding_step() {
    for &(earlier, later, step) in ADR_EXAMPLES {
        assert_eq!(
            compare_names(earlier, later),
            Ordering::Less,
            "{earlier:?} should sort before {later:?}"
        );
        assert_eq!(
            compare_names(later, earlier),
            Ordering::Greater,
            "{later:?} should sort after {earlier:?}"
        );
        let main_key = name_key(earlier).cmp(&name_key(later));
        let expected = if step == DecidingStep::MainKey {
            Ordering::Less
        } else {
            Ordering::Equal
        };
        assert_eq!(
            main_key, expected,
            "{earlier:?} and {later:?} should be decided by {step:?}"
        );
    }
}

#[test]
fn name_order_compares_digit_runs_by_value_at_any_length() {
    let ordered = [
        "file0",
        "file2",
        "file10",
        "file99999999999999999999",
        "file100000000000000000000",
        "file0000000000000000000000000100000000000000000001",
    ];
    for pair in ordered.windows(2) {
        assert_eq!(
            compare_names(pair[0], pair[1]),
            Ordering::Less,
            "{:?} should sort before {:?}",
            pair[0],
            pair[1]
        );
    }
    assert_eq!(
        compare_names("12345678901234567890123", "12345678901234567890124"),
        Ordering::Less
    );
}

#[test]
fn name_order_folds_case_through_unicode_lowercase() {
    assert_eq!(name_key("\u{212A}elvin"), name_key("kelvin"));
    assert_eq!(name_key("ÄRGER"), name_key("ärger"));
    assert_eq!(name_key("\u{130}stanbul"), name_key("i\u{307}stanbul"));
    assert_ne!(name_key("Straße"), name_key("strasse"));
    assert_eq!(compare_names("\u{212A}elvin", "kelvin"), Ordering::Greater);
    assert_eq!(compare_names("ÄRGER", "ärger"), Ordering::Less);
}

#[test]
fn name_order_is_a_strict_total_order_over_edge_case_triples() {
    for &a in EDGE_NAMES {
        for &b in EDGE_NAMES {
            let order = compare_names(a, b);
            assert_eq!(
                order,
                compare_names(b, a).reverse(),
                "antisymmetry fails for {a:?} and {b:?}"
            );
            assert_eq!(
                order == Ordering::Equal,
                a == b,
                "{a:?} and {b:?} must tie only when identical"
            );
            for &c in EDGE_NAMES {
                if order.is_le() && compare_names(b, c).is_le() {
                    assert!(
                        compare_names(a, c).is_le(),
                        "transitivity fails for {a:?}, {b:?}, {c:?}"
                    );
                }
            }
        }
    }
}

fn names_of(output: PipelineData) -> Vec<String> {
    match output {
        PipelineData::Flat(nodes) => nodes.into_iter().map(|node| node.name).collect(),
        PipelineData::Grouped(_) => panic!("Expected Flat output"),
    }
}

fn edge_case_files() -> Vec<NodeEntry> {
    EDGE_NAMES
        .iter()
        .map(|name| make_file(name, 100, false))
        .collect()
}

#[test]
fn test_sort_by_name_uses_the_natural_name_order() {
    let sort = SortBy::new(SortField::Name, SortOrder::Ascending, false);
    let input = ["file10", "Zeta", "file2", "file01", "alpha", "file1", "File1"]
        .into_iter()
        .map(|name| make_file(name, 100, false))
        .collect();

    assert_eq!(
        names_of(sort.process(PipelineData::Flat(input))),
        vec!["alpha", "File1", "file1", "file01", "file2", "file10", "Zeta"]
    );
}

#[test]
fn test_sort_by_name_agrees_with_compare_nodes() {
    let config = PipelineConfig::default().sort(SortField::Name, SortOrder::Ascending, true);
    let mut expected = edge_case_files();
    expected.push(make_dir("dir10", false));
    expected.push(make_dir("Dir2", false));
    expected.reverse();
    let input = expected.clone();
    expected.sort_by(|left, right| crate::pipeline::compare_nodes(&config, left, right));

    let expected: Vec<String> = expected.into_iter().map(|node| node.name).collect();
    assert_eq!(
        names_of(SortBy::from_config(config).process(PipelineData::Flat(input))),
        expected
    );
}

#[test]
fn test_sort_by_name_desc_is_the_exact_reverse_of_ascending() {
    let ascending = SortBy::new(SortField::Name, SortOrder::Ascending, false);
    let descending = SortBy::new(SortField::Name, SortOrder::Descending, false);

    let mut ascending_names = names_of(ascending.process(PipelineData::Flat(edge_case_files())));
    let descending_names = names_of(descending.process(PipelineData::Flat(edge_case_files())));

    ascending_names.reverse();
    assert_eq!(descending_names, ascending_names);
}

#[test]
fn test_equal_sort_values_fall_back_to_ascending_name_order() {
    let names = ["file10.txt", "file2.txt", "File2.txt", "file02.txt"];
    let expected = vec!["File2.txt", "file2.txt", "file02.txt", "file10.txt"];
    let created = SystemTime::UNIX_EPOCH + Duration::from_secs(7);
    let rows = || -> Vec<NodeEntry> {
        names
            .into_iter()
            .map(|name| {
                let mut entry = make_file(name, 100, false);
                entry.created = Some(created);
                entry
            })
            .collect()
    };

    for field in [
        SortField::Size,
        SortField::Modified,
        SortField::Created,
        SortField::Extension,
    ] {
        for order in [SortOrder::Ascending, SortOrder::Descending] {
            let sort = SortBy::new(field, order, false);
            assert_eq!(
                names_of(sort.process(PipelineData::Flat(rows()))),
                expected,
                "{field:?} {order:?} ties should use ascending name order"
            );
        }
    }
}

#[test]
fn test_sort_by_type_uses_the_natural_name_order() {
    let sort = SortBy::new(SortField::Type, SortOrder::Ascending, false);
    let input = ["b10", "B2", "a"]
        .into_iter()
        .map(|name| make_file(name, 100, false))
        .collect();

    assert_eq!(
        names_of(sort.process(PipelineData::Flat(input))),
        vec!["a", "B2", "b10"]
    );
}

#[test]
fn test_grouped_sort_orders_each_group_by_natural_name() {
    let pipeline = Pipeline::from_config(
        &PipelineConfig::default()
            .sort(SortField::Name, SortOrder::Ascending, true)
            .group_by(ConfigGroupBy::Extension),
    );
    let input = ["img10.png", "IMG2.png", "notes10.md", "Notes9.md", "img1.png"]
        .into_iter()
        .map(|name| make_file(name, 100, false))
        .collect();

    let grouped = pipeline.execute_grouped(input);
    let groups: Vec<(String, Vec<String>)> = grouped
        .groups
        .into_iter()
        .map(|group| {
            (
                group.label,
                group.nodes.into_iter().map(|node| node.name).collect(),
            )
        })
        .collect();

    assert_eq!(
        groups,
        vec![
            ("md".to_string(), vec!["Notes9.md".to_string(), "notes10.md".to_string()]),
            (
                "png".to_string(),
                vec![
                    "img1.png".to_string(),
                    "IMG2.png".to_string(),
                    "img10.png".to_string()
                ]
            ),
        ]
    );
}
