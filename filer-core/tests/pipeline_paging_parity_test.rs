//! # Flat and Paged Pipeline Parity
//!
//! Exercises the public scan contract against the same entries that feed the
//! flat pipeline contract. The comparison keeps filtering behavior independent
//! from the paging route selected for a configuration.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use filer_core::model::node::NodeKind;
use filer_core::modules::scan::ScanModule;
use filer_core::pipeline::sort::{SortField, SortOrder};
use filer_core::pipeline::{
    FilterConfig, GroupBy, GroupedEntries, Pipeline, PipelineConfig, PipelinePagingMode,
    compare_nodes,
};
use filer_core::{
    Capabilities, Command, CoreError, DirectoryLoadOptions, DirectoryPageState, DirectoryStream,
    Event, FilerCore, FsProvider, ListingBatch, ListingOptions, Location, LocationRef, NodeEntry,
    NodeEntryCapabilities, NodeMeta, ProviderCx, ProviderPaging, RequestId,
};

const PAGE_SIZE: usize = 2;
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const ROOT: &str = "/pipeline-parity";

#[derive(Clone, Debug, PartialEq, Eq)]
struct RowKey {
    name: String,
    size: u64,
    hidden: bool,
    extension: Option<String>,
}

#[derive(Clone)]
struct FixtureProvider {
    entries: Arc<[NodeEntry]>,
}

impl FixtureProvider {
    fn new() -> Self {
        let entries = vec![
            entry(".hidden.rs", 5, true, Some("rs")),
            entry("visible.rs", 10, false, Some("rs")),
            entry("notes.md", 20, false, Some("md")),
            entry("cache.tmp", 30, false, Some("tmp")),
            entry("report-a.rs", 40, false, Some("rs")),
            entry("report-ab.rs", 50, false, Some("rs")),
            entry("README", 60, false, None),
        ];
        Self {
            entries: entries.into(),
        }
    }
}

struct FixtureStream {
    entries: Arc<[NodeEntry]>,
    next_index: usize,
}

#[async_trait]
impl DirectoryStream for FixtureStream {
    async fn next_batch(
        &mut self,
        max: usize,
        _cx: &ProviderCx<'_>,
    ) -> Result<ListingBatch, CoreError> {
        let end = self
            .next_index
            .saturating_add(max.max(1))
            .min(self.entries.len());
        let entries = self.entries[self.next_index..end].to_vec();
        self.next_index = end;
        if end == self.entries.len() {
            Ok(ListingBatch::final_batch(entries))
        } else {
            Ok(ListingBatch::partial(entries))
        }
    }
}

#[async_trait]
impl FsProvider for FixtureProvider {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    fn paging(&self) -> ProviderPaging {
        ProviderPaging::Fallback
    }

    async fn open_listing(
        &self,
        _path: &Path,
        _options: ListingOptions,
        _cx: &ProviderCx<'_>,
    ) -> Result<Option<Box<dyn DirectoryStream>>, CoreError> {
        Ok(Some(Box::new(FixtureStream {
            entries: self.entries.clone(),
            next_index: 0,
        })))
    }

    async fn list(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        Ok(self.entries.to_vec())
    }

    async fn read(&self, path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }

    async fn read_range(
        &self,
        path: &Path,
        _start: u64,
        _len: u64,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }

    async fn exists(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(false)
    }

    async fn metadata(&self, path: &Path, _cx: &ProviderCx<'_>) -> Result<NodeEntry, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

fn entry(name: &str, size: u64, hidden: bool, extension: Option<&str>) -> NodeEntry {
    let location = Location::local(Path::new(ROOT).join(name));
    NodeEntry {
        location: LocationRef::from_location(&location),
        display_path: None,
        capabilities: NodeEntryCapabilities {
            read: true,
            navigate: false,
        },
        name: name.to_string(),
        kind: NodeKind::File {
            extension: extension.map(str::to_string),
        },
        size,
        modified: None,
        created: None,
        accessed: None,
        meta: NodeMeta {
            hidden,
            ..NodeMeta::default()
        },
    }
}

fn row_keys(entries: &[NodeEntry]) -> Vec<RowKey> {
    entries
        .iter()
        .map(|entry| RowKey {
            name: entry.name.clone(),
            size: entry.size,
            hidden: entry.meta.hidden,
            extension: entry.extension().map(str::to_string),
        })
        .collect()
}

fn grouped_keys(groups: &GroupedEntries) -> Vec<(String, Vec<RowKey>)> {
    groups
        .groups
        .iter()
        .map(|group| (group.label.clone(), row_keys(&group.nodes)))
        .collect()
}

async fn start_core(
    provider: FixtureProvider,
) -> (
    FilerCore,
    flume::Receiver<Event>,
    filer_core::model::session::SessionId,
) {
    let core = FilerCore::new();
    core.load(ScanModule::new(Arc::new(provider)));
    let events = core.event_receiver();
    core.send(Command::Handshake)
        .expect("handshake command should be accepted");
    let session = loop {
        let event = tokio::time::timeout(EVENT_TIMEOUT, events.recv_async())
            .await
            .expect("handshake should arrive before timeout")
            .expect("event channel should stay open");
        if let Event::SessionCreated(session) = event {
            break session;
        }
    };
    (core, events, session)
}

async fn load_page(
    core: &FilerCore,
    events: &flume::Receiver<Event>,
    session: filer_core::model::session::SessionId,
    pipeline: PipelineConfig,
    load: DirectoryLoadOptions,
) -> (GroupedEntries, DirectoryPageState) {
    let request = RequestId::new();
    core.send(Command::Scan {
        location: LocationRef::from_location(&Location::local(ROOT)),
        session,
        pipeline,
        load,
        request,
    })
    .expect("scan command should be accepted");

    loop {
        let event = tokio::time::timeout(EVENT_TIMEOUT, events.recv_async())
            .await
            .expect("page should arrive before timeout")
            .expect("event channel should stay open");
        match event {
            Event::DirectoryPageLoaded {
                groups,
                page,
                request: event_request,
                ..
            } if event_request == request => return (groups, page),
            Event::Error {
                message,
                request: Some(event_request),
                ..
            } if event_request == request => panic!("scan failed: {message}"),
            _ => {}
        }
    }
}

async fn load_all_pages(
    core: &FilerCore,
    events: &flume::Receiver<Event>,
    session: filer_core::model::session::SessionId,
    pipeline: PipelineConfig,
) -> Vec<NodeEntry> {
    let mut cursor = None;
    let mut entries = Vec::new();
    for _ in 0..16 {
        let load = match cursor {
            Some(cursor) => DirectoryLoadOptions::page_after(PAGE_SIZE, cursor),
            None => DirectoryLoadOptions::page(PAGE_SIZE),
        };
        let (groups, page) = load_page(core, events, session, pipeline.clone(), load).await;
        entries.extend(groups.groups.into_iter().flat_map(|group| group.nodes));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return entries,
        }
    }
    panic!("fixture paging did not terminate");
}

#[tokio::test]
async fn flat_and_paged_filters_have_identical_results() {
    let provider = FixtureProvider::new();
    let flat_entries = provider.entries.to_vec();
    let (core, events, session) = start_core(provider).await;
    let cases = [
        (
            "hidden",
            PipelineConfig::default().show_hidden(false),
            PipelinePagingMode::FilteredPage,
        ),
        (
            "include extension",
            PipelineConfig::default().filter(FilterConfig::only_extensions(vec!["rs".into()])),
            PipelinePagingMode::FilteredPage,
        ),
        (
            "exclude extension",
            PipelineConfig::default().filter(FilterConfig::exclude_extensions(vec!["tmp".into()])),
            PipelinePagingMode::FilteredPage,
        ),
        (
            "name",
            PipelineConfig::default().filter(FilterConfig {
                name_pattern: Some("report-?.rs".into()),
                ..FilterConfig::default()
            }),
            PipelinePagingMode::SnapshotOnly,
        ),
        (
            "size",
            PipelineConfig::default().filter(FilterConfig {
                min_size: Some(20),
                max_size: Some(40),
                ..FilterConfig::default()
            }),
            PipelinePagingMode::SnapshotOnly,
        ),
    ];

    for (label, config, expected_mode) in cases {
        assert_eq!(config.paging_mode(), expected_mode, "{label} route");
        let mut expected = Pipeline::from_config(&config).execute_flat(flat_entries.clone());
        if expected_mode == PipelinePagingMode::SnapshotOnly {
            expected.sort_unstable_by(|left, right| compare_nodes(&config, left, right));
        }
        let actual = load_all_pages(&core, &events, session, config).await;
        assert_eq!(row_keys(&actual), row_keys(&expected), "{label} results");
    }

    core.shutdown()
        .await
        .expect("core should shut down after parity checks");
}

#[tokio::test]
async fn paged_sorted_grouped_output_matches_flat_pipeline() {
    let provider = FixtureProvider::new();
    let flat_entries = provider.entries.to_vec();
    let (core, events, session) = start_core(provider).await;
    let config = PipelineConfig::default()
        .sort(SortField::Name, SortOrder::Ascending, true)
        .group_by(GroupBy::Extension);
    let expected = Pipeline::from_config(&config).execute_grouped(flat_entries);
    let (actual, page) = load_page(
        &core,
        &events,
        session,
        config,
        DirectoryLoadOptions::page(32),
    )
    .await;

    assert!(page.complete);
    assert_eq!(grouped_keys(&actual), grouped_keys(&expected));
    core.shutdown()
        .await
        .expect("core should shut down after grouped parity check");
}
