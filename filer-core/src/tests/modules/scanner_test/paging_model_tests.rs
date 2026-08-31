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

    assert_eq!(selection.total_matches, 10_000);
    assert_eq!(selection.entries.len(), 11);
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
