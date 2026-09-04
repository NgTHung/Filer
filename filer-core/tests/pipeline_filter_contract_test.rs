use filer_core::model::node::NodeKind;
use filer_core::pipeline::{FilterConfig, Pipeline, PipelineConfig};
use filer_core::{Location, LocationRef, NodeEntry};

fn file(name: &str, size: u64) -> NodeEntry {
    let location = Location::local(format!("/test/{name}"));
    let mut entry = NodeEntry::from_location_ref(
        LocationRef::from_location(&location),
        name,
        NodeKind::File { extension: None },
    );
    entry.size = size;
    entry
}

#[test]
fn pipeline_applies_inclusive_size_bounds() {
    let config = PipelineConfig::default().filter(FilterConfig {
        min_size: Some(100),
        max_size: Some(200),
        ..Default::default()
    });
    let entries = vec![
        file("below", 99),
        file("minimum", 100),
        file("between", 150),
        file("maximum", 200),
        file("above", 201),
    ];

    let filtered = Pipeline::from_config(&config).execute_flat(entries);
    let names: Vec<_> = filtered.iter().map(|entry| entry.name.as_str()).collect();

    assert_eq!(names, ["minimum", "between", "maximum"]);
}

#[test]
fn pipeline_applies_name_glob() {
    let config = PipelineConfig::default().filter(FilterConfig {
        name_pattern: Some("report-?.*".to_string()),
        ..Default::default()
    });
    let entries = vec![
        file("report-a.txt", 0),
        file("report-b.md", 0),
        file("report-long.txt", 0),
        file("old-report-a.txt", 0),
    ];

    let filtered = Pipeline::from_config(&config).execute_flat(entries);
    let names: Vec<_> = filtered.iter().map(|entry| entry.name.as_str()).collect();

    assert_eq!(names, ["report-a.txt", "report-b.md"]);
}
