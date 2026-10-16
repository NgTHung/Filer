use filer_core::model::node::NodeKind;
use filer_core::model::session::SessionId;
use filer_core::pipeline::EntryGroup;
use filer_core::{
    CoreError, DirectoryCursor, DirectoryPageState, ErrorCode, Event, GroupedEntries, Location,
    LocationRef, NodeEntry, PipelineConfig, RequestId,
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

#[test]
fn selection_survives_refresh_pages_by_location_and_drops_removed_entries_at_completion() {
    let session = SessionId::new();
    let mut model = BrowsingModel::new(session);
    let first = model.load(location("/folder"), PipelineConfig::default());
    model.apply(page(
        session,
        first.request,
        "/folder",
        &["a", "b", "removed"],
    ));
    assert!(model.toggle_selection(1));
    assert!(model.toggle_selection(2));
    assert!(!model.toggle_selection(20));
    model.set_visible_range(1..3);
    let refresh = model.load(
        location("/folder"),
        PipelineConfig::default().show_hidden(true),
    );
    assert_eq!(model.view().unwrap().selected().len(), 2);
    model.apply(with_state(
        page(session, refresh.request, "/folder", &["a"]),
        DirectoryPageState::partial(1, None, DirectoryCursor("refresh".into())),
    ));
    assert_eq!(model.view().unwrap().selected().len(), 2);
    assert_eq!(model.view().unwrap().visible_range(), 1..1);
    let next = model.continue_loading().unwrap();
    let mut result = with_state(
        page(session, next.request, "/folder", &["b"]),
        DirectoryPageState::complete(1, Some(2)).with_window(1),
    );
    if let Event::DirectoryPageLoaded { groups, .. } = &mut result {
        let entry = &mut groups.groups[0].nodes[0];
        entry.location = LocationRef::Id(entry.location.identity());
        entry.name = "different display name".into();
        entry.size = 123;
    }
    model.apply(result);
    assert_eq!(
        model.view().unwrap().selected(),
        &[location("/folder/b").identity()].into()
    );
    assert!(model.toggle_selection(1));
    assert!(model.view().unwrap().selected().is_empty());
    model.toggle_selection(0);
    let other = model.load(location("/other"), PipelineConfig::default());
    model.apply(page(session, other.request, "/other", &["a"]));
    assert!(model.view().unwrap().selected().is_empty());
    assert_eq!(model.view().unwrap().visible_range(), 0..0);
}

#[test]
fn request_failure_preserves_the_usable_view_and_a_fresh_load_recovers() {
    let session = SessionId::new();
    let mut model = BrowsingModel::new(session);
    let first = model.load(location("/folder"), PipelineConfig::default());
    model.apply(page(session, first.request, "/folder", &["keep"]));
    model.toggle_selection(0);
    model.set_visible_range(0..1);
    let failed = model.load(location("/denied"), PipelineConfig::default());
    let error = |session, request| {
        Event::from_request_error(CoreError::permission_denied("/denied"), session, request)
    };
    assert!(!model.apply(error(session, first.request)));
    assert!(!model.apply(error(SessionId::new(), failed.request)));
    assert!(!model.apply(Event::from_error(
        CoreError::permission_denied("/denied"),
        session
    )));
    assert!(!model.apply(Event::SessionCreated(session)));
    assert!(model.apply(error(session, failed.request)));
    assert!(!model.is_loading());
    let failure = model.error().unwrap();
    assert_eq!(failure.request.request, failed.request);
    assert_eq!(failure.code, ErrorCode::PermissionDenied);
    assert!(failure.recoverable);
    assert!(!failure.message.is_empty());
    assert_eq!(model.view().unwrap().visible_entries()[0].name, "keep");
    assert_eq!(model.view().unwrap().selected().len(), 1);
    assert!(!model.apply(page(session, failed.request, "/denied", &["late"])));
    let retry = model.load(location("/folder"), PipelineConfig::default());
    assert!(model.error().is_none());
    assert!(model.apply(page(session, retry.request, "/folder", &[])));
    assert!(model.view().unwrap().selected().is_empty());
    assert_eq!(model.view().unwrap().visible_range(), 0..0);
}
