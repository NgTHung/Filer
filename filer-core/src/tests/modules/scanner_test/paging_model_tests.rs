use crate::modules::scan::PageSelection;
use crate::pipeline::PipelineConfig;

#[test]
fn selection_retains_only_page_size_plus_lookahead() {
    let config = PipelineConfig::default();
    let mut selection = PageSelection::new(10, None, &config);
    let entries = (0..10_000)
        .map(|index| FileNode {
            id: NodeId(index),
            name: format!("{index:05}.txt"),
            path: PathBuf::from(format!("/tmp/{index:05}.txt")),
            kind: NodeKind::File {
                extension: Some("txt".into()),
            },
            size: index,
            modified: None,
            created: None,
            accessed: None,
            meta: NodeMeta::default(),
        })
        .collect();

    selection.extend(entries);

    assert_eq!(selection.total_matches, 10_000);
    assert_eq!(selection.entries.len(), 11);
}
