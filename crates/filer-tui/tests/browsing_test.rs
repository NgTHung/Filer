use filer_core::model::node::NodeKind;
use filer_core::model::session::SessionId;
use filer_core::pipeline::EntryGroup;
use filer_core::{
    DirectoryCursor, DirectoryPageState, Event, GroupedEntries, Location, LocationRef, NodeEntry,
    PipelineConfig, RequestId,
};
use filer_tui::BrowsingModel;

fn location(path: &str) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

fn with_state(mut event: Event, state: DirectoryPageState) -> Event {
    if let Event::DirectoryPageLoaded { page, .. } = &mut event {
        *page = state;
    }
    event
}

fn page(session: SessionId, request: RequestId, parent: &str, names: &[&str]) -> Event {
    Event::DirectoryPageLoaded {
        parent: location(parent),
        groups: GroupedEntries {
            total_count: names.len(),
            groups: vec![EntryGroup {
                label: String::new(),
                order: 0,
                nodes: names
                    .iter()
                    .map(|name| {
                        NodeEntry::from_location_ref(
                            location(&format!("{parent}/{name}")),
                            *name,
                            NodeKind::File { extension: None },
                        )
                    })
                    .collect(),
            }],
        },
        page: DirectoryPageState::complete(names.len(), Some(names.len())),
        session,
        request,
    }
}

#[test]
fn navigation_replacement_accepts_only_the_pending_session_request_and_location() {
    let session = SessionId::new();
    let mut model = BrowsingModel::new(session);
    let first = model.load(location("/first"), PipelineConfig::with_default_sort());
    assert_eq!(model.session(), session);
    assert!(model.is_loading());
    assert!(model.apply(page(session, first.request, "/first", &["z", "a"])));
    assert_eq!(model.view().unwrap().entries()[0].name, "z");

    let second = model.load(location("/second"), PipelineConfig::default());
    let third = model.load(location("/third"), PipelineConfig::default());
    assert_eq!(third.replaces, Some(second.request));
    assert_eq!(model.pending().unwrap().request, third.request);
    assert!(!model.apply(page(session, first.request, "/first", &["late"])));
    assert!(!model.apply(page(session, second.request, "/second", &["late"])));
    assert!(!model.apply(page(
        SessionId::new(),
        third.request,
        "/third",
        &["foreign"]
    )));
    assert!(!model.apply(page(session, third.request, "/wrong", &["wrong"])));
    assert_eq!(
        model.view().unwrap().location().identity(),
        location("/first").identity()
    );
    assert!(model.apply(page(session, third.request, "/third", &[])));
    assert!(model.view().unwrap().entries().is_empty());
    assert!(!model.is_loading());
    assert!(model.view().unwrap().pages()[0].complete);
    assert!(!model.apply(page(session, third.request, "/third", &["duplicate"])));
}

#[test]
fn continuations_are_requested_once_and_append_in_core_order() {
    let session = SessionId::new();
    let mut model = BrowsingModel::new(session);
    assert!(model.continue_loading().is_none());
    let pipeline = PipelineConfig::with_default_sort().show_hidden(true);
    let first = model.load(location("/folder"), pipeline.clone());
    model.apply(with_state(
        page(session, first.request, "/folder", &["z", "a"]),
        DirectoryPageState::partial(2, None, DirectoryCursor("next".into())),
    ));
    model.set_visible_range(1..10);
    assert_eq!(model.view().unwrap().visible_range(), 1..2);
    let next = model.continue_loading().unwrap();
    assert_eq!(next.cursor, Some(DirectoryCursor("next".into())));
    assert_eq!(next.pipeline, pipeline);
    assert_ne!(next.request, first.request);
    assert!(model.continue_loading().is_none());
    assert!(!model.apply(page(session, first.request, "/folder", &["duplicate"])));
    let last = with_state(
        page(session, next.request, "/folder", &["b"]),
        DirectoryPageState::complete(1, Some(3)).with_window(2),
    );
    assert!(model.apply(last.clone()));
    assert!(!model.apply(last));
    let view = model.view().unwrap();
    assert_eq!(
        view.entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["z", "a", "b"]
    );
    assert_eq!(view.pages().len(), 2);
    assert_eq!(view.pipeline(), &pipeline);
    assert_eq!(view.visible_range(), 1..2);
    model.set_visible_range(1..10);
    assert_eq!(model.view().unwrap().visible_entries().len(), 2);
    model.set_visible_range(10..1);
    assert_eq!(model.view().unwrap().visible_range(), 3..3);
    assert!(model.continue_loading().is_none());
}
