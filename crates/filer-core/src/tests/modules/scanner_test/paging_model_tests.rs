use crate::modules::scan::PageSelection;
use crate::pipeline::PipelineConfig;
use crate::ProviderCx;

#[test]
fn selection_retains_only_page_size_plus_lookahead() {
    let config = PipelineConfig::default();
    let mut selection = PageSelection::with_lookahead(10, 0, None, &config);
    let entries: Vec<_> = (0..10_000)
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

    assert!(selection.extend(entries, &ProviderCx::none()));

    let selected = selection.finish();
    assert_eq!(selected.total_matches, 10_000);
    assert_eq!(selected.entries.len(), 11);
}

#[test]
fn selection_stops_after_periodic_cancellation_check() {
    let config = PipelineConfig::default();
    let cancel = crate::CancelSignal::new();
    let context = ProviderCx::with_cancel(&cancel);
    let cancel_during_iteration = cancel.clone();
    let mut selection = PageSelection::with_lookahead(10, 0, None, &config);

    let completed = selection.extend(
        (0..10_000).map(move |index| {
            if index == 300 {
                cancel_during_iteration.cancel();
            }
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
        }),
        &context,
    );

    assert!(!completed);
    assert!(selection.total_matches < 1_000);
}

fn natural_row(name: &str) -> NodeEntry {
    local_file_node(
        format!("/tmp/natural/{name}"),
        name,
        NodeKind::File {
            extension: Some("txt".into()),
        },
        0,
        None,
        NodeMeta::default(),
    )
}

#[test]
fn selection_keeps_rows_after_the_keyset_boundary_in_natural_name_order() {
    let config = PipelineConfig::default();
    let names = ["file10.txt", "File2.txt", "file1.txt", "file2.txt", "file02.txt", "Zeta.txt"];
    let mut selection =
        PageSelection::with_lookahead(3, 0, Some(natural_row("File2.txt")), &config);

    assert!(selection.extend(names.map(natural_row), &ProviderCx::none()));

    let selected = selection.finish();
    assert_eq!(selected.total_matches, names.len());
    assert_eq!(
        selected
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["file2.txt", "file02.txt", "file10.txt", "Zeta.txt"]
    );
}
