use crate::modules::scan::PageSelection;
use crate::pipeline::PipelineConfig;
use crate::tests::fixtures::local_file_node;

#[test]
fn selection_retains_only_page_size_plus_lookahead() {
    let config = PipelineConfig::default();
    let mut selection = PageSelection::new(10, None, &config);
    let entries = (0..10_000)
        .map(|index| {
            local_file_node(
                format!("/tmp/{index:05}.txt"),
                format!("{index:05}.txt"),
                NodeKind::File {
                extension: Some("txt".into()),
                },
                index,
                None,
                NodeMeta::default(),
            )
        })
        .collect();

    selection.extend(entries);

    assert_eq!(selection.total_matches, 10_000);
    assert_eq!(selection.entries.len(), 11);
}
