//! # Large Directory Paging Contract
//!
//! Proves the public scan command preserves bounded provider paging before
//! timing data is used to judge large-directory performance.
//!
//! ```
//! use filer_core::DirectoryLoadOptions;
//!
//! let load = DirectoryLoadOptions::page(256);
//! assert!(load.is_paged());
//! ```

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use filer_core::modules::scan::ScanModule;
use filer_core::{
    Capabilities, Command, CoreError, DirectoryLoadOptions, DirectoryStream, Event, FilerCore,
    FsProvider, ListingBatch, ListingOptions, Location, LocationRef, NodeEntry,
    NodeEntryCapabilities, NodeMeta, PipelineConfig, ProviderCx, ProviderPaging, RequestId,
};

const ENTRY_COUNT: usize = 10_000;
const PAGE_SIZE: usize = 256;
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

struct CountingProvider {
    entries: Arc<[NodeEntry]>,
    full_list_calls: Arc<AtomicUsize>,
    stream_rows_yielded: Arc<AtomicUsize>,
    stream_reached_end: Arc<AtomicUsize>,
}

impl CountingProvider {
    fn new(entry_count: usize) -> Self {
        let parent = Path::new("/benchmark");
        let entries: Vec<NodeEntry> = (0..entry_count)
            .map(|index| {
                let name = format!("entry_{index:05}.dat");
                let path = parent.join(&name);
                NodeEntry {
                    location: LocationRef::from_location(&Location::local(path)),
                    display_path: None,
                    capabilities: NodeEntryCapabilities {
                        read: true,
                        navigate: false,
                    },
                    name,
                    kind: filer_core::model::node::NodeKind::File {
                        extension: Some("dat".to_string()),
                    },
                    size: 0,
                    modified: None,
                    created: None,
                    accessed: None,
                    meta: NodeMeta::default(),
                }
            })
            .collect();

        Self {
            entries: entries.into(),
            full_list_calls: Arc::new(AtomicUsize::new(0)),
            stream_rows_yielded: Arc::new(AtomicUsize::new(0)),
            stream_reached_end: Arc::new(AtomicUsize::new(0)),
        }
    }
}

struct CountingStream {
    entries: Arc<[NodeEntry]>,
    next_index: usize,
    rows_yielded: Arc<AtomicUsize>,
    reached_end: Arc<AtomicUsize>,
}

#[async_trait]
impl DirectoryStream for CountingStream {
    async fn next_batch(
        &mut self,
        max: usize,
        _cx: &ProviderCx<'_>,
    ) -> Result<ListingBatch, CoreError> {
        if max == 0 {
            return Ok(ListingBatch::partial(Vec::new()));
        }
        let end = self.next_index.saturating_add(max).min(self.entries.len());
        let entries = self.entries[self.next_index..end].to_vec();
        self.next_index = end;
        self.rows_yielded
            .fetch_add(entries.len(), Ordering::Relaxed);
        if end == self.entries.len() {
            self.reached_end.store(1, Ordering::Relaxed);
            Ok(ListingBatch::final_batch(entries))
        } else {
            Ok(ListingBatch::partial(entries))
        }
    }
}

#[async_trait]
impl FsProvider for CountingProvider {
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
        ProviderPaging::Native
    }

    async fn open_listing(
        &self,
        _path: &Path,
        _options: ListingOptions,
        _cx: &ProviderCx<'_>,
    ) -> Result<Option<Box<dyn DirectoryStream>>, CoreError> {
        Ok(Some(Box::new(CountingStream {
            entries: self.entries.clone(),
            next_index: 0,
            rows_yielded: self.stream_rows_yielded.clone(),
            reached_end: self.stream_reached_end.clone(),
        })))
    }

    async fn list(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        self.full_list_calls.fetch_add(1, Ordering::Relaxed);
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

#[tokio::test]
async fn first_page_through_public_command_does_not_materialize_full_listing() {
    let provider = Arc::new(CountingProvider::new(ENTRY_COUNT));
    let core = FilerCore::new();
    core.load(ScanModule::new(provider.clone()));
    let events = core.event_receiver();

    core.send(Command::Handshake)
        .expect("handshake command should be accepted");
    let session = loop {
        let event = tokio::time::timeout(EVENT_TIMEOUT, events.recv_async())
            .await
            .expect("handshake event should arrive before timeout")
            .expect("event channel should stay open");
        if let Event::SessionCreated(session) = event {
            break session;
        }
    };

    let request = RequestId::new();
    core.send(Command::Scan {
        location: LocationRef::from_location(&Location::local("/benchmark")),
        session,
        pipeline: PipelineConfig::default(),
        load: DirectoryLoadOptions::page(PAGE_SIZE),
        request,
    })
    .expect("scan command should be accepted");

    let page = loop {
        let event = tokio::time::timeout(EVENT_TIMEOUT, events.recv_async())
            .await
            .expect("page event should arrive before timeout")
            .expect("event channel should stay open");
        match event {
            Event::DirectoryPageLoaded {
                page,
                request: event_request,
                ..
            } if event_request == request => break page,
            Event::Error {
                message,
                request: Some(event_request),
                ..
            } if event_request == request => panic!("scan failed: {message}"),
            _ => {}
        }
    };

    assert_eq!(page.page_count, PAGE_SIZE);
    assert!(!page.complete);
    assert!(page.next_cursor.is_some());
    assert_eq!(provider.full_list_calls.load(Ordering::Relaxed), 0);
    let stream_rows = provider.stream_rows_yielded.load(Ordering::Relaxed);
    assert!(
        (PAGE_SIZE..ENTRY_COUNT).contains(&stream_rows),
        "the first public page should stop before the directory ends, observed {stream_rows} of {ENTRY_COUNT} provider rows"
    );
    assert_eq!(provider.stream_reached_end.load(Ordering::Relaxed), 0);

    core.shutdown()
        .await
        .expect("core should shut down after the paging proof");
}
